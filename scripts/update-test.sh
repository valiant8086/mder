#!/bin/sh
# Sets up a test of mder's updater on Linux, with nothing published, like
# update-test.ps1 on Windows:
#
#   FOLDER/app/      an "old" mder, set to look for updates at SOURCE
#   FOLDER/server/   a "new" release: its tar.gz and changelog.json
#
#   scripts/update-test.sh FOLDER [SOURCE]
#   python3 -m http.server 8086 --directory FOLDER/server
#
# SOURCE defaults to http://127.0.0.1:8086/. OLD_VERSION and NEW_VERSION
# default to 0.1.0 and 0.1.1. Put other systems' downloads in FOLDER/server
# too, to serve their tests from here.
set -eu

cd "$(dirname "$0")/.."
folder=$1
source=${2:-http://127.0.0.1:8086/}
old=${OLD_VERSION:-0.1.0}
new=${NEW_VERSION:-0.1.1}

MDER_VERSION=$old scripts/linux-pack.sh
rm -rf "$folder/app"
mkdir -p "$folder"
tar -xzf "dist/mder$old-linux-"*.tar.gz -C "$folder"
mv "$folder/mder" "$folder/app"
# Due for a daily check straight away, and pointed at the test server.
printf 'check_for_updates=yes\nlast_update_check=0\nupdate_source=%s\n' "$source" > "$folder/app/mder.ini"

MDER_VERSION=$new scripts/linux-pack.sh
mkdir -p "$folder/server"
cp "dist/mder$new-linux-"*.tar.gz "$folder/server/"

# The repo's changelog, with the test version on top.
python3 - "$new" "$folder/server/changelog.json" <<'PY'
import datetime, json, sys
new, out = sys.argv[1], sys.argv[2]
entries = json.load(open("changelog.json", encoding="utf-8"))
today = datetime.date.today()
test = {"version": new, "date": f"{today.month}/{today.day}/{today.year}", "blurb": "A test update.",
        "changes": [f"A test release, {new}, to try mder's updater.", "Nothing else changed."]}
json.dump([test] + entries, open(out, "w", encoding="utf-8"), indent=2)
PY

echo "Old mder $old is in $folder/app; release $new is in $folder/server."
echo "Serve it with: python3 -m http.server 8086 --directory $folder/server"
