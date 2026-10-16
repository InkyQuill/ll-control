use super::*;
use rustix::fs::{flock, FlockOperation};
use std::{
    fs::{self, OpenOptions},
    path::Path,
};

#[tokio::test]
async fn lost_settings_caller_keeps_io_owned_and_shutdown_waits_for_the_actual_file_transaction() {
    let root = tempfile::tempdir().unwrap();
    let app_root = root.path().join("app");
    fs::create_dir(&app_root).unwrap();
    let lock_path = app_root.join("desktop-settings.lock");
    let held = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .unwrap();
    flock(&held, FlockOperation::LockExclusive).unwrap();
    let store = Store::new(
        app_root,
        root.path().join("config"),
        std::env::current_exe().unwrap(),
    )
    .unwrap();
    let desktop = Arc::new(Desktop {
        store,
        preferences: Mutex::new(None),
        ready: AtomicBool::new(false),
        tray_error: Mutex::new(None),
        closed: AtomicBool::new(false),
        jobs: Arc::new(tokio::sync::Semaphore::new(JOBS as usize)),
    });
    let wanted = Preferences {
        autostart: true,
        start_minimized: true,
        close_to_tray: false,
    };
    let caller = {
        let d = desktop.clone();
        let wanted = wanted.clone();
        tokio::spawn(async move { d.save(0, wanted).await })
    };
    // Detect the real blocking Store worker's second open descriptor, not just
    // asynchronous admission. It is waiting on our advisory filesystem lock.
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let count = fs::read_dir("/proc/self/fd")
                .unwrap()
                .filter_map(Result::ok)
                .filter(|e| fs::read_link(e.path()).ok().as_deref() == Some(Path::new(&lock_path)))
                .count();
            if count >= 2 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    caller.abort();
    assert!(caller.await.unwrap_err().is_cancelled());
    assert_eq!(desktop.jobs.available_permits(), JOBS as usize - 1);
    let stopping = {
        let d = desktop.clone();
        tokio::spawn(async move { d.shutdown().await })
    };
    tokio::time::timeout(Duration::from_secs(5), async {
        while !desktop.closed.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(
        !stopping.is_finished(),
        "Exit abandoned an admitted file transaction"
    );
    drop(held);
    tokio::time::timeout(Duration::from_secs(5), stopping)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let persisted = desktop.store.read().unwrap();
    assert_eq!(persisted.preferences, wanted);
    assert_eq!(persisted.registration.enabled, Some(true));
    assert_eq!(desktop.preferences().unwrap(), wanted);
    assert!(desktop.clone().read().await.is_err());
    assert!(desktop
        .clone()
        .save(persisted.revision, Preferences::default())
        .await
        .is_err());
}
