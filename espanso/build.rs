#[cfg(windows)]
fn main() {
    use std::{env, fs, path::PathBuf};

    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is not set"));
    let icon_path = manifest_dir.join("../assets/respanso.ico");
    let output_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is not set"));
    let resource_file = output_dir.join("respanso-icon.rc");
    let icon_resource_path = icon_path.to_string_lossy().replace('\\', "/");

    println!("cargo:rerun-if-changed={}", icon_path.display());
    fs::write(&resource_file, format!("1 ICON \"{icon_resource_path}\"\n"))
        .expect("failed to create the rEspanso icon resource file");

    let mut resource = winres::WindowsResource::new();
    resource.set_resource_file(
        resource_file
            .to_str()
            .expect("rEspanso icon resource path is not valid UTF-8"),
    );
    resource
        .compile()
        .expect("failed to embed the rEspanso Windows icon");
}

#[cfg(not(windows))]
fn main() {}
