#!/bin/sh
# Builds a release of mder for Linux and packs it into dist/, which git
# ignores: dist/mder0.2-linux-x64.tar.gz (or -arm64), named for the
# version's first two parts and the processor it was built on, holding a
# folder mder/ with the program, the README and the
# licenses. mder uses the system's GTK 3 and WebKitGTK 4.1; the README
# lists the packages.
#
#   scripts/linux-pack.sh
#
# Set MDER_VERSION to build and name it as another version, for testing the
# updater.
set -eu

cd "$(dirname "$0")/.."

cargo build --release

version=${MDER_VERSION:-$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)}
short=$(echo "$version" | cut -d. -f1-2)
stage=dist/linux
rm -rf "$stage"
mkdir -p "$stage/mder"
cp target/release/mder README.md LICENSE-MIT.txt LICENSE-APACHE.txt NOTICE.txt "$stage/mder/"

case "$(uname -m)" in
    x86_64) arch=x64 ;;
    aarch64 | arm64) arch=arm64 ;;
    *) arch=$(uname -m) ;;
esac
tarball="dist/mder$short-linux-$arch.tar.gz"
rm -f "$tarball"
tar -czf "$tarball" -C "$stage" mder
echo "Built $tarball, version $version ($(du -h "$tarball" | cut -f1))"
