#!/bin/sh
# Builds mder and wraps it into mder.app, so macOS treats it as a real app:
# its own menu bar, coming to the front properly, and offering mder for
# markdown files in Finder's Open With menu.
#
#   scripts/macos-app.sh                    # release build: target/release/mder.app
#   scripts/macos-app.sh --debug            # debug build: target/debug/mder.app
#   scripts/macos-app.sh --universal        # Apple silicon and Intel in one app:
#                                           #   target/universal/release/mder.app
#   scripts/macos-app.sh --universal --zip  # and packs it into dist/ for a release,
#                                           #   named for the whole version:
#                                           #   dist/mder0.2.44-mac-universal.zip
#
# --universal needs Rust's targets for both: rustup target add aarch64-apple-darwin x86_64-apple-darwin
# Set MDER_VERSION to build and name it as another version, for testing the updater.
#
# Open it with: open target/release/mder.app --args path/to/file.md
set -eu

cd "$(dirname "$0")/.."

profile=release
universal=no
zip=no
for arg in "$@"; do
    case "$arg" in
        --debug) profile=debug ;;
        --universal) universal=yes ;;
        --zip) zip=yes ;;
        *) echo "Unknown option: $arg" >&2; exit 2 ;;
    esac
done
cargo_flags=
if [ "$profile" = release ]; then
    cargo_flags=--release
fi

if [ "$universal" = yes ]; then
    out="target/universal/$profile"
    for target in aarch64-apple-darwin x86_64-apple-darwin; do
        cargo build $cargo_flags --target "$target"
    done
    mkdir -p "$out"
    lipo -create -output "$out/mder" \
        "target/aarch64-apple-darwin/$profile/mder" "target/x86_64-apple-darwin/$profile/mder"
    arch_name=universal
else
    cargo build $cargo_flags
    out="target/$profile"
    arch_name=$(uname -m)
fi

version=${MDER_VERSION:-$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)}
app="$out/mder.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$out/mder" "$app/Contents/MacOS/mder"
cp README.md LICENSE-MIT.txt LICENSE-APACHE.txt NOTICE.txt "$app/Contents/Resources/"

# The markdown extensions here should match KINDS in src/integration/mod.rs.
# LSHandlerRank Alternate: offer mder for these files, never claim to be the default.
# macOS's own Markdown type covers only .md and .markdown, and an app can't
# add to it, so the other extensions are claimed by name in a second entry.
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>mder</string>
    <key>CFBundleDisplayName</key><string>mder</string>
    <key>CFBundleIdentifier</key><string>com.valiant8086.mder</string>
    <key>CFBundleExecutable</key><string>mder</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>$version</string>
    <key>CFBundleVersion</key><string>$version</string>
    <key>LSMinimumSystemVersion</key><string>12.0</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSPrincipalClass</key><string>NSApplication</string>
    <key>CFBundleDocumentTypes</key>
    <array>
        <dict>
            <key>CFBundleTypeName</key><string>Markdown document</string>
            <key>CFBundleTypeRole</key><string>Editor</string>
            <key>LSHandlerRank</key><string>Alternate</string>
            <key>LSItemContentTypes</key>
            <array><string>net.daringfireball.markdown</string></array>
        </dict>
        <dict>
            <key>CFBundleTypeName</key><string>Markdown document</string>
            <key>CFBundleTypeRole</key><string>Editor</string>
            <key>LSHandlerRank</key><string>Alternate</string>
            <key>CFBundleTypeExtensions</key>
            <array>
                <string>mdown</string><string>mkd</string><string>mkdn</string><string>mdwn</string>
            </array>
        </dict>
    </array>
</dict>
</plist>
PLIST

# Sign it locally (ad hoc), so macOS runs it without complaining that the
# bundle and its program don't match.
codesign --force --deep --sign - "$app"

# Tell Launch Services about it, so Open With can offer it. Not for a
# release zip: a second registered copy here could change which one opens.
lsregister=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister
if [ "$zip" = no ] && [ -x "$lsregister" ]; then
    "$lsregister" -f "$app"
fi

echo "Built $app"

if [ "$zip" = yes ]; then
    mkdir -p dist
    zip_path="dist/mder$version-mac-$arch_name.zip"
    rm -f "$zip_path"
    # ditto keeps the bundle intact, as Finder's own Compress does.
    ditto -c -k --keepParent "$app" "$zip_path"
    echo "Packed $zip_path"
fi

echo "Open it with: open $app --args path/to/file.md"
