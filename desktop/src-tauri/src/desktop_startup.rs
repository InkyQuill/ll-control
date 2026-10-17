//! Graphics compatibility before GTK/WebKit or asynchronous runtimes start.

/// Use the established WebKit fallback on NVIDIA Wayland sessions unless the
/// user explicitly selected a renderer. Re-exec avoids mutating the environment
/// of a multithreaded process and preserves arguments, PID and exit semantics.
/// Recording workers must be dispatched before calling this function.
#[cfg(target_os = "linux")]
pub fn configure_graphics() -> std::io::Result<()> {
    use std::os::unix::process::CommandExt;
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some()
        && std::env::var_os("GDK_BACKEND").is_none_or(|value| value != "x11");
    if !wayland
        || !std::path::Path::new("/sys/module/nvidia").exists()
        || std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_some()
    {
        return Ok(());
    }
    // Observed on Plasma Wayland: explicit-sync buffer attachment without an
    // acquire point terminates GTK before a window is usable. The variable also
    // guards re-entry, including explicit user opt-out with value 0.
    let error = std::process::Command::new(std::env::current_exe()?)
        .args(std::env::args_os().skip(1))
        .env("WEBKIT_DISABLE_DMABUF_RENDERER", "1")
        .exec();
    Err(error)
}

#[cfg(not(target_os = "linux"))]
pub fn configure_graphics() -> std::io::Result<()> {
    Ok(())
}
