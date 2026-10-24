fn main() {
    tauri_build::build();

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os == "windows" && target_env == "gnu" {
        // The GNU linker otherwise exports every symbol from the Tauri cdylib.
        // Large dependency graphs can exceed the PE export-ordinal limit.
        println!("cargo:rustc-link-arg-cdylib=-Wl,--exclude-all-symbols");
    }
}
