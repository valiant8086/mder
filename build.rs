//! Sets mder's version, and embeds a Windows manifest: current common
//! controls (needed for the tree view's half-checked boxes and visual
//! styles), per-monitor DPI awareness, UTF-8 as the code page, and long path
//! support.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    // MDER_VERSION: Cargo.toml's version, unless the environment gives
    // another, so a test build can pose as a newer release for the updater.
    println!("cargo:rerun-if-env-changed=MDER_VERSION");
    let version = std::env::var("MDER_VERSION")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| std::env::var("CARGO_PKG_VERSION").unwrap_or_default());
    println!("cargo:rustc-env=MDER_VERSION={version}");
    if std::env::var("CARGO_CFG_WINDOWS").is_ok() {
        use embed_manifest::manifest::{ActiveCodePage, DpiAwareness, Setting, SupportedOS::*};
        use embed_manifest::{embed_manifest, new_manifest};

        let manifest = new_manifest("mder")
            .supported_os(Windows7..=Windows10)
            .active_code_page(ActiveCodePage::Utf8)
            .dpi_awareness(DpiAwareness::PerMonitorV2)
            .long_path_aware(Setting::Enabled);
        if let Err(e) = embed_manifest(manifest) {
            println!("cargo:warning=Couldn't embed the Windows manifest: {e}");
        }
    }
}
