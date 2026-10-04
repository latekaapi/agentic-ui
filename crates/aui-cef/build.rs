fn main() {
    println!("cargo:rerun-if-changed=src/shim.m");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    cc::Build::new().file("src/shim.m").flag("-fobjc-arc").flag("-Wno-unused-parameter").compile("aui_cef_shim");
    println!("cargo:rustc-link-lib=framework=AppKit");
    println!("cargo:rustc-link-lib=framework=CoreFoundation");
}
