//! Gives the Windows executable its icon and version details, so Explorer,
//! the taskbar and Task Manager show the app as itself.

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut resource = winresource::WindowsResource::new();
    resource
        .set_icon("assets/icon.ico")
        .set("ProductName", "Youtube Music Spotified")
        .set("FileDescription", "Youtube Music Spotified");
    // A missing resource compiler costs the icon, not the build.
    if let Err(error) = resource.compile() {
        println!("cargo:warning=the executable has no icon: {error}");
    }
}
