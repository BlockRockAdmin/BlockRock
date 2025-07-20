fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(true)
        .out_dir("src")
        .file_descriptor_set_path("src/blockrock_descriptor.bin")
        .compile_protos(&["proto/zion.proto"], &["proto"])?;
    Ok(())
}
