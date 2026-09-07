fn main() {
    let attributes = tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[lattice_core::GET_APP_INFO_COMMAND]),
    );

    if let Err(error) = tauri_build::try_build(attributes) {
        eprintln!("failed to prepare Lattice desktop build metadata: {error}");
        std::process::exit(1);
    }
}
