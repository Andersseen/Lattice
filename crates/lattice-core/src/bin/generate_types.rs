use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/types/src/generated.ts");

    fs::write(output_path, lattice_core::typescript_bindings())?;

    Ok(())
}
