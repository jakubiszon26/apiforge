//! REL-02 (#119): the Windows resource of `apiforge-desktop.exe`: the application icon,
//! which Explorer and the taskbar show for the file, and the version strings
//! of Explorer's *Details* tab. Built only when Windows builds for Windows,
//! with the Windows SDK's `rc.exe`; on any other host or target this part
//! does nothing, so Linux builds need neither `winresource` nor `rc.exe`.
//!
//! KTLO-44 (#325): the module declarations of the tickets' window tests, one
//! per file of `src/folder/acceptance_tests/`, so that adding a ticket's file
//! edits no shared file.

use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../packaging/icon/apiforge.ico");
    windows_resource();
    acceptance_modules();
}

/// The directory of the tickets' window tests, relative to the package.
const ACCEPTANCE_TESTS: &str = "src/folder/acceptance_tests";

/// Writes `acceptance_modules.rs` to `OUT_DIR`: a `#[path] mod` pair for each
/// `.rs` file of [`ACCEPTANCE_TESTS`], sorted by name. The paths are absolute
/// because a module declared in an included file resolves against the
/// includer, `src/folder/acceptance_tests.rs`.
fn acceptance_modules() {
    println!("cargo:rerun-if-changed={ACCEPTANCE_TESTS}");
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    let mut declarations = String::new();
    for file in ticket_files(&Path::new(&manifest).join(ACCEPTANCE_TESTS)) {
        let stem = file.file_stem().and_then(|stem| stem.to_str());
        let path = file.to_str();
        let (Some(stem), Some(path)) = (stem, path) else {
            panic!("{} is not a UTF-8 module path", file.display());
        };
        declarations.push_str(&format!("#[path = {path:?}]\nmod {stem};\n"));
    }
    let out = std::env::var("OUT_DIR").expect("cargo sets OUT_DIR");
    std::fs::write(Path::new(&out).join("acceptance_modules.rs"), declarations)
        .expect("OUT_DIR is writable");
}

/// The `.rs` files of `directory`, sorted by name.
fn ticket_files(directory: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("{}: {error}", directory.display()))
        .map(|entry| entry.expect("a readable directory entry").path())
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "rs"))
        .collect();
    files.sort();
    files
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
