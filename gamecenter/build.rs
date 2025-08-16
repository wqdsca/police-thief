use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);

    // Auth service proto 컴파일
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .out_dir(&out_dir)
        .compile(&["proto/auth.proto"], &["proto"])?;

    println!("cargo:rerun-if-changed=proto/auth.proto");

    Ok(())
}
