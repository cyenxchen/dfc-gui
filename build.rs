fn main() {
    // Generate protobuf types for iothub payload decoding.
    // Note: proto files are intentionally minimal (see `proto/`).
    println!("cargo:rerun-if-changed=proto/DataType.proto");
    println!("cargo:rerun-if-changed=proto/DataTypeCOMM.proto");

    prost_build::Config::new()
        .compile_protos(&["proto/DataType.proto", "proto/DataTypeCOMM.proto"], &["proto"])
        .expect("Failed to compile protobuf definitions");

    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.compile().expect("Failed to compile Windows resources");
    }
}
