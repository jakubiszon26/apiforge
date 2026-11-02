//! REL-02 (#119): the Windows resource of `apiforge-desktop.exe`: the application icon,
//! which Explorer and the taskbar show for the file, and the version strings
//! of Explorer's *Details* tab. Built only when Windows builds for Windows,
//! with the Windows SDK's `rc.exe`; on any other host or target this script
//! does nothing, so Linux builds need neither `winresource` nor `rc.exe`.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../packaging/icon/apiforge.ico");
    windows_resource();
}

#[cfg(windows)]
fn windows_resource() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let version = std::env::var("CARGO_PKG_VERSION").expect("cargo sets CARGO_PKG_VERSION");
    winresource::WindowsResource::new()
        .set_icon("../../packaging/icon/apiforge.ico")
        .set("ProductName", "API Forge")
        .set("FileDescription", "API Forge")
        .set("FileVersion", &version)
        .set("ProductVersion", &version)
        .compile_for("apiforge-desktop")
        .expect("the Windows resource of apiforge-desktop.exe compiles");
}

#[cfg(not(windows))]
fn windows_resource() {}
