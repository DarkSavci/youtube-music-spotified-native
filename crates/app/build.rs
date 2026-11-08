//! Gives the Windows executable its icon and version details, so Explorer,
//! the taskbar and Task Manager show the app as itself; and in a working
//! tree, builds the Go core the app runs.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=build.rs");
    build_core();
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

/// Builds the core into `target/core/`, where a development build looks for
/// it (`sidecar::locate`), whenever its Go code changes.
///
/// Built by hand before, with `scripts/build-core`, it was easily forgotten:
/// a core a month old ran under every change to the Go code made since, and
/// nothing said so. Skipped when cross-compiling (the packaging scripts
/// build the core for each target), with `SPOTIFIED_SKIP_CORE_BUILD` set,
/// and outside a working tree; a missing Go is a warning, not a failure.
fn build_core() {
    println!("cargo:rerun-if-env-changed=SPOTIFIED_SKIP_CORE_BUILD");
    if std::env::var_os("SPOTIFIED_SKIP_CORE_BUILD").is_some() {
        return;
    }
    if std::env::var("TARGET").ok() != std::env::var("HOST").ok() {
        return;
    }
    let Some(manifest) = std::env::var_os("CARGO_MANIFEST_DIR").map(PathBuf::from) else {
        return;
    };
    let core = manifest.join("../../core");
    if !core.join("go.mod").exists() {
        return;
    }
    // Cargo looks through the whole folder for a change.
    println!("cargo:rerun-if-changed={}", core.display());
    let Some(target) = target_dir() else {
        return;
    };
    let windows = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");
    let binary = target.join("core").join(if windows {
        "spotified-core.exe"
    } else {
        "spotified-core"
    });
    let built = Command::new("go")
        .current_dir(&core)
        .env("CGO_ENABLED", "0")
        .args(["build", "-o"])
        .arg(&binary)
        .arg("./cmd/spotifier")
        .status();
    match built {
        Ok(status) if status.success() => {}
        Ok(status) => println!(
            "cargo:warning=the Go core did not build ({status}); the app runs the one at {}, \
             which may be out of date",
            binary.display()
        ),
        Err(error) => println!(
            "cargo:warning=the Go core was not built ({error}): is Go installed? The app runs \
             the one at {}, which may be out of date",
            binary.display()
        ),
    }
}

/// The folder the executable lands in is `<target>/<profile>`, and the build
/// script's own output is `<target>/<profile>/build/<crate>/out`.
fn target_dir() -> Option<PathBuf> {
    let out = PathBuf::from(std::env::var_os("OUT_DIR")?);
    out.ancestors().nth(4).map(Path::to_path_buf)
}
