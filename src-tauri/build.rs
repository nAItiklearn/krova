fn main() {
    // Replace tauri's default Windows manifest with ours: same Common Controls v6
    // dependency, plus per-monitor DPI awareness v2 and an explicit "asInvoker".
    println!("cargo:rerun-if-changed=windows/app.manifest");
    let windows =
        tauri_build::WindowsAttributes::new().app_manifest(include_str!("windows/app.manifest"));
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("failed to run the tauri build step");
}
