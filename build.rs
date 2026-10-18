//! Embeds a Windows manifest: current common controls (needed for the tree
//! view's half-checked boxes and visual styles), per-monitor DPI awareness,
//! UTF-8 as the code page, and long path support.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
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
