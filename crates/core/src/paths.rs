//! Where Ultron keeps its files (ADR-005). On Linux these are exactly the original's
//! paths, so the Python and Rust versions share one set of data; on Windows and macOS
//! they are the platform's own data and config folders.
use std::path::PathBuf;

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_default()
}

/// `$ULTRON_DATA`, else `~/.local/share/ultron` on Linux, else the platform data dir.
pub fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("ULTRON_DATA").filter(|v| !v.is_empty()) {
        return PathBuf::from(dir);
    }
    if cfg!(target_os = "linux") {
        home().join(".local/share/ultron")
    } else {
        dirs::data_dir().unwrap_or_else(home).join("Ultron")
    }
}

/// `~/.config/ultron` on Linux, else the platform config dir.
pub fn config_dir() -> PathBuf {
    if cfg!(target_os = "linux") {
        home().join(".config/ultron")
    } else {
        dirs::config_dir().unwrap_or_else(home).join("Ultron")
    }
}

/// The shared SQLite file: sessions, memory, tasks.
pub fn db() -> PathBuf {
    data_dir().join("ultron.db")
}

/// Write a file atomically (temp file + rename), readable only by the user.
pub fn write_private(path: &std::path::Path, contents: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    std::fs::create_dir_all(dir)?;
    let mut tmp = tempfile_in(dir)?;
    tmp.1.write_all(contents)?;
    tmp.1.sync_all()?;
    std::fs::rename(&tmp.0, path)
}

fn tempfile_in(dir: &std::path::Path) -> std::io::Result<(PathBuf, std::fs::File)> {
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    for n in 0..100u32 {
        let p = dir.join(format!(".ultron-{}-{n}.tmp", std::process::id()));
        match opts.open(&p) {
            Ok(f) => return Ok((p, f)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::other("no free temp file name"))
}
