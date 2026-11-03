//! COM's exports are found by name and must stay out of the import library,
//! which only a module definition file can say.
//!
//! The Windows APIs are bound from the ones `bindings.txt` names rather than
//! taken from the `windows` crate, so `windows-core` follows its own releases.

fn main() {
    println!("cargo:rerun-if-changed=kanaemi.def");
    println!("cargo:rerun-if-changed=bindings.txt");
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        let def = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("kanaemi.def");
        println!("cargo:rustc-cdylib-link-arg=/DEF:{}", def.display());
    }
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let out = std::env::var("OUT_DIR").expect("cargo sets OUT_DIR");
        windows_bindgen::builder()
            .output(format!("{out}/bindings.rs"))
            .filter_file("bindings.txt")
            .flat()
            .write();
    }
}
