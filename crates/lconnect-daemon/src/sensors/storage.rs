//! Mounted filesystem capacity via isolated util-linux findmnt JSON.
//! Source semantics and limitations: docs/development/parity/sensor-provider-inventory.md.
use super::reading;
use lconnect_ipc::{SensorReading, SensorStatus, SensorUnit};
use serde::Deserialize;
use std::{collections::BTreeMap, path::PathBuf, process::Stdio, time::Duration};
use tokio::{
    io::AsyncReadExt,
    process::{Child, Command},
};

const LIMIT: usize = 262_144;
const ARGS: [&str; 7] = [
    "--json",
    "--bytes",
    "--list",
    "--df",
    "--real",
    "--nofsroot",
    "--output=SOURCE,FSTYPE,UUID,FSROOT,MAJ:MIN,TARGET,SIZE,USED,AVAIL",
];
pub(super) struct Storage {
    executable: PathBuf,
    previous: Vec<SensorReading>,
    draining: Option<Child>,
}
impl Storage {
    pub(super) fn new(executable: PathBuf) -> Self {
        Self {
            executable,
            previous: Vec::new(),
            draining: None,
        }
    }
    pub(super) async fn collect(&mut self, now: u64) -> Vec<SensorReading> {
        match self.query().await.and_then(|bytes| parse(&bytes, now)) {
            Ok(rows) => {
                self.previous = rows.clone();
                rows
            }
            Err(status) => {
                let mut rows: Vec<_> = self
                    .previous
                    .iter()
                    .map(|r| reading(&r.id, &r.label, r.unit, Err(status), now))
                    .collect();
                rows.push(reading(
                    "storage:discovery",
                    "Filesystem capacity unavailable",
                    SensorUnit::Bytes,
                    Err(status),
                    now,
                ));
                rows
            }
        }
    }
    async fn query(&mut self) -> Result<Vec<u8>, SensorStatus> {
        if let Some(child) = self.draining.as_mut() {
            match child.try_wait() {
                Ok(Some(_)) => self.draining = None,
                _ => return Err(SensorStatus::Unavailable),
            }
        }
        let mut child = Command::new(&self.executable)
            .args(ARGS)
            .env("LC_ALL", "C")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| SensorStatus::Unavailable)?;
        let stdout = child.stdout.take().ok_or(SensorStatus::Unavailable)?;
        let operation = async {
            let mut bytes = Vec::new();
            stdout
                .take((LIMIT + 1) as u64)
                .read_to_end(&mut bytes)
                .await
                .map_err(|_| SensorStatus::Unavailable)?;
            if bytes.len() > LIMIT {
                return Err(SensorStatus::Invalid);
            }
            if !child
                .wait()
                .await
                .map_err(|_| SensorStatus::Unavailable)?
                .success()
            {
                return Err(SensorStatus::Unavailable);
            }
            Ok(bytes)
        };
        let result = tokio::time::timeout(Duration::from_millis(750), operation)
            .await
            .unwrap_or(Err(SensorStatus::Unavailable));
        if result.is_err() {
            let _ = child.start_kill();
            // A filesystem syscall can remain in uninterruptible kernel sleep.
            // Never block the collector indefinitely or spawn a second helper.
            if !matches!(
                tokio::time::timeout(Duration::from_millis(100), child.wait()).await,
                Ok(Ok(_))
            ) {
                self.draining = Some(child);
            }
        }
        result
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    filesystems: Vec<Filesystem>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Filesystem {
    source: String,
    fstype: String,
    uuid: Option<String>,
    fsroot: String,
    #[serde(rename = "maj:min")]
    device: String,
    target: String,
    size: serde_json::Value,
    used: serde_json::Value,
    avail: serde_json::Value,
}
fn count(value: &serde_json::Value) -> Result<u64, SensorStatus> {
    match value {
        serde_json::Value::Null => Err(SensorStatus::Unavailable),
        serde_json::Value::Number(n) => n.as_u64().ok_or(SensorStatus::Invalid),
        serde_json::Value::String(s) if s == "-" || s.is_empty() => Err(SensorStatus::Unavailable),
        serde_json::Value::String(s) => s.parse().map_err(|_| SensorStatus::Invalid),
        _ => Err(SensorStatus::Invalid),
    }
}
fn hex(s: &str) -> String {
    s.as_bytes().iter().map(|b| format!("{b:02x}")).collect()
}
fn parse(bytes: &[u8], now: u64) -> Result<Vec<SensorReading>, SensorStatus> {
    if bytes.len() > LIMIT {
        return Err(SensorStatus::Invalid);
    }
    let mut catalog: Catalog = serde_json::from_slice(bytes).map_err(|_| SensorStatus::Invalid)?;
    if catalog.filesystems.len() > 128 {
        return Err(SensorStatus::Invalid);
    }
    // Duplicate bind mounts select the lexically first target consistently.
    catalog.filesystems.sort_by(|a, b| a.target.cmp(&b.target));
    let mut seen = BTreeMap::new();
    let mut rows = Vec::new();
    for fs in catalog.filesystems {
        if fs.source.is_empty()
            || fs.fstype.is_empty()
            || !fs.fsroot.starts_with('/')
            || !fs.target.starts_with('/')
            || fs.target.len() > 4096
        {
            return Err(SensorStatus::Invalid);
        }
        let (major, minor) = fs.device.split_once(':').ok_or(SensorStatus::Invalid)?;
        if major.parse::<u32>().is_err() || minor.parse::<u32>().is_err() {
            return Err(SensorStatus::Invalid);
        }
        let volume = fs
            .uuid
            .filter(|v| !v.is_empty())
            .map(|v| format!("uuid-{}", hex(&v)))
            .unwrap_or_else(|| format!("source-{}", hex(&fs.source)));
        let id = format!("storage:{volume}:{}:{}", hex(&fs.fstype), hex(&fs.fsroot));
        if id.len() > 980 {
            return Err(SensorStatus::Invalid);
        }
        if let Some(device) = seen.insert(id.clone(), fs.device.clone()) {
            if device != fs.device {
                return Err(SensorStatus::Invalid);
            }
            continue;
        }
        let size = count(&fs.size);
        let used = count(&fs.used).and_then(|used| match size {
            Ok(size) if used > size => Err(SensorStatus::Invalid),
            _ => Ok(used),
        });
        let free = size.and_then(|size| {
            used.and_then(|used| size.checked_sub(used).ok_or(SensorStatus::Invalid))
        });
        let available = count(&fs.avail).and_then(|avail| match free {
            Ok(free) if avail > free => Err(SensorStatus::Invalid),
            _ => Ok(avail),
        });
        let percent = size.and_then(|size| {
            if size == 0 {
                Err(SensorStatus::Unavailable)
            } else {
                used.map(|used| 100.0 * used as f64 / size as f64)
            }
        });
        let label: String = fs.target.chars().take(96).collect();
        for (metric, unit, value) in [
            ("total", SensorUnit::Bytes, size.map(|v| v as f64)),
            ("used", SensorUnit::Bytes, used.map(|v| v as f64)),
            ("free", SensorUnit::Bytes, free.map(|v| v as f64)),
            ("available", SensorUnit::Bytes, available.map(|v| v as f64)),
            ("used_percent", SensorUnit::Percent, percent),
        ] {
            rows.push(reading(
                format!("{id}:{metric}"),
                format!("{label}: {metric}"),
                unit,
                value,
                now,
            ));
        }
    }
    rows.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;
    fn volume(target: &str) -> serde_json::Value {
        serde_json::json!({"source":"/dev/a","fstype":"ext4","uuid":"test-uuid","fsroot":"/","maj:min":"8:1","target":target,"size":1000,"used":400,"avail":500})
    }
    fn catalog(volumes: Vec<serde_json::Value>) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({"filesystems":volumes})).unwrap()
    }
    #[test]
    fn capacity_reserved_space_identity_and_duplicate_mounts() {
        let first = parse(&catalog(vec![volume("/space name")]), 1).unwrap();
        let get = |rows: &[SensorReading], suffix: &str| {
            rows.iter().find(|r| r.id.ends_with(suffix)).unwrap().value
        };
        assert_eq!(get(&first, ":total"), Some(1000.0));
        assert_eq!(get(&first, ":used"), Some(400.0));
        assert_eq!(get(&first, ":free"), Some(600.0));
        assert_eq!(get(&first, ":available"), Some(500.0));
        assert_eq!(get(&first, ":used_percent"), Some(40.0));
        let moved = parse(&catalog(vec![volume("/new"), volume("/bind")]), 2).unwrap();
        assert_eq!(moved.len(), 5);
        assert_eq!(
            first.iter().map(|r| &r.id).collect::<Vec<_>>(),
            moved.iter().map(|r| &r.id).collect::<Vec<_>>()
        );
        let mut sub = volume("/sub");
        sub["fsroot"] = "/subvolume".into();
        assert_eq!(
            parse(&catalog(vec![volume("/"), sub]), 3).unwrap().len(),
            10
        );
        let mut clone = volume("/clone");
        clone["maj:min"] = "8:2".into();
        assert!(parse(&catalog(vec![volume("/"), clone]), 3).is_err());
        assert!(parse(&catalog(vec![]), 4).unwrap().is_empty());
    }
    #[test]
    fn malformed_missing_and_zero_capacity_are_not_fake_usage() {
        for (size, used, avail) in [
            (
                serde_json::json!(100),
                serde_json::json!(200),
                serde_json::json!(0),
            ),
            (
                serde_json::json!(0),
                serde_json::json!(0),
                serde_json::json!(0),
            ),
            (
                serde_json::Value::Null,
                serde_json::Value::Null,
                serde_json::Value::Null,
            ),
        ] {
            let mut fs = volume("/");
            fs["size"] = size;
            fs["used"] = used;
            fs["avail"] = avail;
            let rows = parse(&catalog(vec![fs]), 1).unwrap();
            assert!(rows
                .iter()
                .find(|r| r.id.ends_with(":used_percent"))
                .unwrap()
                .value
                .is_none());
        }
        let mut fs = volume("/");
        fs["avail"] = serde_json::json!(700);
        assert_eq!(
            parse(&catalog(vec![fs]), 1)
                .unwrap()
                .iter()
                .find(|r| r.id.ends_with(":available"))
                .unwrap()
                .status,
            SensorStatus::Invalid
        );
        assert!(parse(&catalog(vec![volume("/"); 129]), 1).is_err());
        assert!(parse(&vec![b' '; LIMIT + 1], 1).is_err());
        assert!(parse(br#"{"filesystems":[],"unknown":true}"#, 1).is_err());
    }
    fn fixture(script: &str) -> (TempDir, Storage) {
        let dir = TempDir::new().unwrap();
        let exe = dir.path().join("query");
        std::fs::write(&exe, format!("#!/bin/sh\n{script}\n")).unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o700)).unwrap();
        let p = Storage::new(exe);
        (dir, p)
    }
    #[tokio::test]
    async fn query_arguments_failure_recovery_and_disappearance() {
        let data = String::from_utf8(catalog(vec![volume("/")])).unwrap();
        let (dir, mut provider) = fixture(&format!(
            "[ \"$*\" = '{}' ] || exit 1\n[ \"$LC_ALL\" = C ] || exit 1\nprintf '%s' '{}'",
            ARGS.join(" "),
            data
        ));
        let first = provider.collect(1).await;
        assert_eq!(first.len(), 5);
        assert!(first.iter().all(|r| r.status == SensorStatus::Valid));
        std::fs::write(dir.path().join("query"), "#!/bin/sh\nexit 1\n").unwrap();
        let failed = provider.collect(2).await;
        assert_eq!(failed.len(), 6);
        assert!(failed.iter().all(|r| r.value.is_none()));
        std::fs::write(
            dir.path().join("query"),
            "#!/bin/sh\nprintf '%s' '{\"filesystems\":[]}'\n",
        )
        .unwrap();
        assert!(provider.collect(3).await.is_empty());
    }
    #[tokio::test]
    async fn timeout_and_oversize_do_not_hold_collector() {
        let (dir, mut provider) = fixture("echo $$ > \"$0.pid\"; exec sleep 10");
        let at = std::time::Instant::now();
        assert_eq!(
            provider.collect(1).await[0].status,
            SensorStatus::Unavailable
        );
        assert!(at.elapsed() < Duration::from_secs(2));
        let pid = std::fs::read_to_string(dir.path().join("query.pid")).unwrap();
        assert!(!std::path::Path::new(&format!("/proc/{}", pid.trim())).exists());
        std::fs::write(
            dir.path().join("query"),
            format!("#!/bin/sh\nexec head -c {} /dev/zero\n", LIMIT + 1),
        )
        .unwrap();
        assert_eq!(provider.collect(2).await[0].status, SensorStatus::Invalid);
    }
    #[tokio::test]
    async fn unreaped_helper_prevents_additional_processes() {
        let (dir, mut provider) = fixture("echo started > \"$0.started\"; exit 1");
        provider.draining = Some(
            Command::new("sleep")
                .arg("10")
                .kill_on_drop(true)
                .spawn()
                .unwrap(),
        );
        assert_eq!(
            provider.collect(1).await[0].status,
            SensorStatus::Unavailable
        );
        assert!(!dir.path().join("query.started").exists());
        let mut child = provider.draining.take().unwrap();
        child.kill().await.unwrap();
    }
    #[tokio::test]
    async fn capacity_reaches_collector_cache_and_lcd_formatting() {
        use super::super::{spawn, LinuxSensors, SensorCache};
        let mut fs = volume("/data");
        fs["size"] = serde_json::json!(4294967296u64);
        fs["used"] = serde_json::json!(2147483648u64);
        fs["avail"] = serde_json::json!(1073741824u64);
        let data = String::from_utf8(catalog(vec![fs])).unwrap();
        let (dir, storage) = fixture(&format!("printf '%s' '{data}'"));
        let mut provider = LinuxSensors::new(dir.path().join("hwmon"), dir.path().join("proc"));
        provider.storage = Some(storage);
        let cache = SensorCache::default();
        let (tx, rx) = tokio::sync::watch::channel(false);
        let task = spawn(provider, cache.clone(), rx);
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let rows = cache.readings();
                if let Some(row) = rows
                    .iter()
                    .find(|r| r.id.starts_with("storage:") && r.id.ends_with(":used"))
                {
                    assert_eq!(
                        lconnect_ipc::theme_sensor_text(
                            &rows,
                            &row.id,
                            SensorUnit::Bytes,
                            lconnect_ipc::TemperatureUnit::Celsius,
                            1
                        ),
                        "2.0 GiB"
                    );
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        tx.send(true).unwrap();
        tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .unwrap()
            .unwrap();
    }
}
