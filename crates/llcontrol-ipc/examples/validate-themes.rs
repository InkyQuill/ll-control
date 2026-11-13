//! Validate frontend-generated presets against the actual daemon wire schema.
use std::io::Read;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    std::io::stdin().take(65537).read_to_end(&mut bytes)?;
    if bytes.len() > 65536 {
        return Err("theme contract input exceeds 64 KiB".into());
    }
    let themes: Vec<llcontrol_ipc::LcdTheme> = serde_json::from_slice(&bytes)?;
    if themes.is_empty() {
        return Err("no presets supplied".into());
    }
    for theme in &themes {
        theme.validate()?;
    }
    println!(
        "Validated {} frontend themes against Rust schema",
        themes.len()
    );
    Ok(())
}
