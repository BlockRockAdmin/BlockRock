fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR")?);
    tonic_build::configure()
        .build_server(true)
        .file_descriptor_set_path(out_dir.join("blockrock_descriptor.bin"))
        .compile_protos(&["proto/zion.proto"], &["proto"])?;
    Ok(())
}
