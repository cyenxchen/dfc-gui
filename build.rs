use std::path::Path;

fn main() {
    // Generate protobuf types for iothub payload decoding.
    // Note: proto files are intentionally minimal (see `proto/`).
    println!("cargo:rerun-if-changed=proto/DataType.proto");
    println!("cargo:rerun-if-changed=proto/DataTypeCOMM.proto");
    println!("cargo:rerun-if-changed=assets/icon.ico");

    let mut prost_config = prost_build::Config::new();
    if let Ok(protoc) = protoc_bin_vendored::protoc_bin_path() {
        prost_config.protoc_executable(protoc);
    } else {
        println!("cargo:warning=Vendored protoc unavailable, falling back to protoc from PATH");
    }

    prost_config
        .compile_protos(
            &["proto/DataType.proto", "proto/DataTypeCOMM.proto"],
            &["proto"],
        )
        .expect("Failed to compile protobuf definitions");

    #[cfg(target_os = "windows")]
    {
        if Path::new("assets/icon.ico").exists() {
            let mut res = winres::WindowsResource::new();
            res.set_icon("assets/icon.ico");
            res.compile().expect("Failed to compile Windows resources");
        } else {
            println!(
                "cargo:warning=Skipping Windows icon embedding because assets/icon.ico was not found"
            );
        }
    }
}
