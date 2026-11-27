fn main() {
    // Tauri checks the bundled Ultron Hands (a sidecar) exists. Release builds put the real
    // binary there; for everyday builds an empty placeholder is enough (the app then uses
    // an `ultron-hands` on PATH).
    let target = std::env::var("TARGET").unwrap_or_default();
    let ext = if target.contains("windows") { ".exe" } else { "" };
    let sidecar = std::path::PathBuf::from(format!("binaries/ultron-hands-{target}{ext}"));
    if !sidecar.exists() {
        let _ = std::fs::create_dir_all("binaries");
        let _ = std::fs::write(&sidecar, b"");
    }
    tauri_build::build()
}
