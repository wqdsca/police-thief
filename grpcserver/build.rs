fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_client(true)
        .build_server(true)
        .out_dir("src/proto") // 생성 위치
        .compile(
            &["proto/room.proto", "proto/user.proto"],
            &["proto"],
        )?;
    Ok(())
}
