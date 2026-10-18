//! Read-only fontconfig catalogue. The webview receives opaque IDs, never a file picker path.
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, io::Read, path::PathBuf, process::Command};

#[derive(Clone, Serialize)]
pub struct SystemFont {
    pub id: String,
    pub name: String,
}
fn parse_catalogue(raw: &str) -> BTreeMap<String, (String, PathBuf)> {
    let mut fonts = BTreeMap::new();
    for line in raw.lines() {
        let mut fields = line.split('\t');
        let (Some(file), Some(family), Some(style), Some(index)) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        // The renderer loads the default face; named variable instances cannot be selected.
        if index != "0" {
            continue;
        }
        let path = PathBuf::from(file);
        if !path.is_absolute()
            || !matches!(
                path.extension()
                    .and_then(|s| s.to_str())
                    .map(str::to_ascii_lowercase)
                    .as_deref(),
                Some("ttf" | "otf")
            )
        {
            continue;
        }
        let name = format!(
            "{} · {}",
            family.split(',').next().unwrap_or(family),
            style.split(',').next().unwrap_or(style)
        );
        let id = format!("{:x}", Sha256::digest(file.as_bytes()));
        fonts.insert(id, (name, path));
    }
    fonts
}
fn catalogue() -> Result<BTreeMap<String, (String, PathBuf)>, String> {
    let output = Command::new("fc-list")
        .arg("--format=%{file}\t%{family}\t%{style}\t%{index}\n")
        .output()
        .map_err(|e| format!("Fontconfig: {e}"))?;
    if !output.status.success() {
        return Err("Fontconfig could not list installed fonts".into());
    }
    Ok(parse_catalogue(&String::from_utf8_lossy(&output.stdout)))
}
pub fn list() -> Result<Vec<SystemFont>, String> {
    let mut fonts: Vec<_> = catalogue()?
        .into_iter()
        .map(|(id, (name, _))| SystemFont { id, name })
        .collect();
    fonts.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
    Ok(fonts)
}
pub fn read(id: &str) -> Result<Vec<u8>, String> {
    if id.len() != 64
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("Unknown installed font ID".into());
    }
    let mut catalogue = catalogue()?;
    let (_, path) = catalogue
        .remove(id)
        .ok_or("Installed font is no longer available")?;
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("Font is not a regular file".into());
    }
    let mut bytes = Vec::new();
    file.take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.is_empty() || bytes.len() > 16 * 1024 * 1024 {
        return Err("Font must be between 1 byte and 16 MiB".into());
    }
    Ok(bytes)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalogue_filters_unsupported_collections_and_deduplicates_files() {
        let fonts=parse_catalogue("/fonts/a.ttf\tAlpha,Alternate\tRegular,Normal\t0\n/fonts/a.ttf\tAlpha\tRegular\t0\n/fonts/b.ttc\tCollection\tRegular\t0\nrelative.otf\tRelative\tRegular\t0\n/fonts/variable.ttf\tNamed\tBold\t65536\n/fonts/c.OTF\tGamma\tBold\t0\n");
        assert_eq!(fonts.len(), 2);
        assert!(fonts.values().any(|(name, _)| name == "Alpha · Regular"));
        assert!(fonts.keys().all(|id| id.len() == 64));
    }
    #[test]
    fn arbitrary_path_is_not_a_catalogue_id() {
        let fonts = parse_catalogue("/fonts/a.ttf\tAlpha\tRegular\t0\n");
        assert!(!fonts.contains_key("/fonts/a.ttf"));
        assert!(read("/fonts/a.ttf").is_err());
    }
}
