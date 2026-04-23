#[cfg(target_os = "windows")]
fn windows_version_value(version: &str) -> Option<u64> {
    let core_version = version.split(['-', '+']).next().unwrap_or(version);
    let mut parts = core_version.split('.');

    let major = parts.next()?.parse::<u16>().ok()?;
    let minor = parts.next().unwrap_or("0").parse::<u16>().ok()?;
    let patch = parts.next().unwrap_or("0").parse::<u16>().ok()?;
    let build = parts.next().unwrap_or("0").parse::<u16>().ok()?;

    Some(((major as u64) << 48) | ((minor as u64) << 32) | ((patch as u64) << 16) | (build as u64))
}

fn main() {
    // Generate protobuf types for iothub payload decoding.
    // Note: proto files are intentionally minimal (see `proto/`).
    println!("cargo:rerun-if-changed=proto/DataType.proto");
    println!("cargo:rerun-if-changed=proto/DataTypeCOMM.proto");
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=assets/icon.png");
    println!("cargo:rerun-if-changed=icons/dfc-gui.icns");

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
        let icon_path = "assets/icon.ico";
        if std::path::Path::new(icon_path).exists() {
            let package_version =
                std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".to_string());
            let mut res = winres::WindowsResource::new();
            res.set_icon(icon_path)
                .set("ProductName", "DFC-GUI")
                .set(
                    "FileDescription",
                    "DFC GUI - A native GUI client for device fleet control",
                )
                .set("InternalName", "dfc-gui")
                .set("OriginalFilename", "dfc-gui.exe")
                .set("CompanyName", "Goldwind DFC Team")
                .set(
                    "LegalCopyright",
                    "Copyright 2026 Goldwind DFC Team. All rights reserved.",
                )
                .set("ProductVersion", &package_version)
                .set("FileVersion", &package_version);

            if let Some(version_info) = windows_version_value(&package_version) {
                res.set_version_info(winres::VersionInfo::PRODUCTVERSION, version_info);
                res.set_version_info(winres::VersionInfo::FILEVERSION, version_info);
                println!(
                    "cargo:warning=Embedding Windows icon and VERSIONINFO resources from {icon_path} for {package_version}"
                );
            } else {
                println!(
                    "cargo:warning=Could not parse CARGO_PKG_VERSION={package_version} into numeric Windows VERSIONINFO; embedding icon resource only"
                );
            }

            res.compile().expect("Failed to compile Windows resources");
        } else {
            println!(
                "cargo:warning=Skipping Windows icon embedding because {icon_path} was not found"
            );
        }
    }
}
