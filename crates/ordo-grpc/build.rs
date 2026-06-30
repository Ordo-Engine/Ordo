fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Compile proto files
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .out_dir("src/generated")
        .compile(&["proto/ordo.proto"], &["proto"])?;

    // Rerun if proto files change
    println!("cargo:rerun-if-changed=proto/ordo.proto");

    Ok(())
}
