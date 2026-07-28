#[cfg(windows)]
fn main() {
    println!("cargo:rerun-if-changed=../assets/respanso.ico");
    let mut resource = winres::WindowsResource::new();
    resource.set_icon("../assets/respanso.ico");
    resource
        .compile()
        .expect("failed to embed the rEspanso Match Studio Windows icon");
}

#[cfg(not(windows))]
fn main() {}
