# mder

A markdown editor and renderer built for screen readers, NVDA first.

The window has two panes: a plain text editor for the markdown source, and a rendered view you read in browse mode. They stay in sync:

- F6 switches panes. It's a toggle.
  - Going to the rendered view puts the screen reader on the block for the editor's caret line.
  - Going back to the editor doesn't move the editor's caret. To move the caret to where you are in the rendered view, press Enter there instead.
  - So if you read ahead in the rendered view and press F6, the editor is still where you left it, and the next F6 takes you to the editor's line in the rendered view, not to where you were reading. Something to be mindful of.
  - On Linux with Orca, F6 from the rendered view does take the editor to where you're reading, because Orca lets the page know. See below.
- Enter on a block in the rendered view takes you to that line in the editor. Clicking a block with the mouse does the same.
- Links in the rendered view only work within the document, such as links to headings. Links to websites and other files don't open anything yet.
- The rendered view updates as you type, patching only what changed so the screen reader keeps its place.

So to keep both positions in step while running a screen reader, use F6 to go from the editor to the rendered view, and Enter to go from the rendered view to the editor. It works this way because a web page can't tell where a screen reader is reading. Screen readers read web pages from their own copy of the content, so moving around doesn't touch the page itself. Pressing Enter makes the screen reader click the block you're on, which the page can see, so that's the solution. Orca is the exception: it moves the page's text cursor as you read, so with Orca, F6 keeps both positions in step both ways. How the syncing works, and what we learned about NVDA in web views along the way, is in [docs/nvda-findings.md](docs/nvda-findings.md).

Also, if another program changes the file, mder reloads it. With unsaved edits, it asks first.

## Getting it

Download `mder0.1-windows-x64.exe` (the number is the version; `-arm64` for Windows on ARM) from the [releases page](https://github.com/valiant8086/mder/releases) and run it. It asks which folder to unpack mder into; then run `mder.exe` from there. mder is a single portable exe; nothing gets installed. It needs the WebView2 runtime, which ships with Windows 10 and 11.

To offer mder when you open markdown files from Explorer (or your Linux file manager), use Tools, Options, or run `mder --register` once. See [Open with](#open-with) below.

On a Mac, download `mder0.1-mac-universal.zip`, which runs on both Apple silicon and Intel Macs, and unzip it. Move mder.app where you want to keep it, such as Applications, before opening it. mder isn't signed by Apple, so the first time you open it, macOS refuses: go to System Settings, Privacy & Security, and choose "Open Anyway". After that it opens normally. It needs macOS 12 or later.

On Linux, download `mder0.1-linux-x64.tar.gz` (or `-arm64`), unpack it (`tar -xzf mder0.1-linux-x64.tar.gz` makes a folder named `mder`), and run `mder/mder`. It uses your system's GTK 3 and WebKitGTK 4.1, which most desktops already have; on Debian or Ubuntu, the packages are `libgtk-3-0` and `libwebkit2gtk-4.1-0`. Built on Debian 13; tested there with MATE and on Ubuntu 26.04 with GNOME.

## Command line

```
mder [--edit | --view] [--line N] [--find TEXT] [--anchor SLUG] [--log FILE] [FILE[:LINE][#SLUG]]
mder --register
mder --unregister
```

- `mder notes.md` opens the file.
- `mder notes.md:42` or `mder --line 42 notes.md` goes to line 42.
- `mder notes.md#setup-steps` or `mder --anchor setup-steps notes.md` goes to the heading with that anchor (GitHub style: lowercase, punctuation dropped, spaces as hyphens).
- `mder --find "some text" notes.md` goes to the first line containing that text. An exact match wins, otherwise the match ignores case.
- `--edit` opens in the editor, and `--view` in the rendered view, whatever the settings say. See [Which pane a file opens in](#which-pane-a-file-opens-in).
- `--log FILE` writes a debug log to that file, for troubleshooting. See [Debug log](#debug-log).
- `mder --register` offers mder for every file type it supports, and `mder --unregister` removes it again. See [Open with](#open-with).

Each file opens in one window. Running mder again for a file that's already open in another instance of it sends the location (if provided) to that window, brings it to the front and exits with code 0. The window reloads the file from disk first. Exit code 1 means the open window didn't respond.

This makes mder easy to drive from another program or AI coding agent. For example, after changing a file, Claude could run `mder --find "the changed text" file.md` to show you the exact context in the rendered view. Just pressing enter is all it takes to then make your own edits.

On macOS, launch it through `open`, so it starts as an app and can come to the front, and pass full paths:

```
open -n /path/to/mder.app --args --find "the changed text" /path/to/file.md
```

`-n` starts a new copy each time, which hands the location to the window that has the file, as on Windows. Opening files from Finder works without any of this.

### Which pane a file opens in

Tools, Options, on the General tab, has two settings, both set to the rendered view at first:

- **When opening a file, start in:** for a plain open, such as from Explorer or `mder notes.md`.
- **When a command opens a file at a location, start in:** for an open with a line, heading or `--find`. This applies even if the location isn't found; mder just doesn't move anywhere new.

`--edit` or `--view` overrides both for that launch. A file that's already open stays in whichever pane you're in, and moves to the location there, unless `--edit` or `--view` says otherwise. With no file, mder starts in the editor.

Settings are saved in `mder.ini`, next to mder.exe, so mder stays portable. If that folder can't be written to, they go in `%APPDATA%\mder` instead. On macOS, Settings is in the mder menu (Command+Comma), and the file is in `~/Library/Application Support/mder`.

### Debug log

`mder --log C:\path\mder.log notes.md` makes that window add a line to the log file whenever it gets a location to go to, moves the editor's caret, puts the screen reader on a block in the rendered view, or notices the file changed on disk. Each line starts with mder's process number, so lines from different windows can be told apart. Without `--log`, mder writes no log at all.

If the file is already open, the window that has it starts logging to that file too, if it wasn't logging already. A later Options setting will turn the log on for every launch; `--log` will override it for that launch.

### Open with

On Windows, registering adds mder to the list in the "How do you want to open this file?" dialog. In Explorer, open a markdown file's context menu, choose Open with, then "Choose another app", and mder is in that list. mder never changes which program opens a file type by default. If you want markdown files to always open in mder, check "Always use this app" in that dialog.

- **Tools, Options**, on the Integration tab, has a checklist of file types, grouped by kind. Each box shows whether that type is registered right now. Change them and press OK or Apply.
- **`mder --register`** does the same for every type at once, and **`mder --unregister`** removes them all. Run it from wherever you keep mder.

Registering writes to your user's part of the registry, so it needs no admin rights. It records where mder is, so if you move mder, register it again from its new place. Tools, Options shows any type still pointing at the old copy, and checking it moves it to this one.

On macOS, mder.app is always in Finder's Open With menu for markdown files. The Integration tab in Settings instead chooses which types open in mder when you double-click them. Checking one makes mder the default and remembers the app it replaced; unchecking gives the type back to that app, or asks which app to use. macOS asks you to confirm each change.

On Linux, registering works as on Windows: mder joins the Open with list in your file manager, and never changes which program opens a file type by default. Linux desktops go by file type rather than extension, so the Integration tab has one box for all markdown. Registering writes `mder.desktop` to `~/.local/share/applications`, and also teaches your desktop that .mdown, .mkdn and .mdwn are markdown (some systems call them plain text), with `mder.xml` in `~/.local/share/mime/packages`. Unchecking the box removes both. If no default program is set for markdown, the desktop may open it in mder on double-click, since mder names markdown more closely than general text editors do; choose a default in your file manager's Open with dialog to change that. `--register` and `--unregister` work here too.

## Building

You need Rust, and on Windows the Visual Studio C++ build tools with CMake and Ninja (both come with Visual Studio's C++ workload). wxDragon downloads prebuilt wxWidgets on the first build.

```
cargo build --release
```

`scripts\dist.ps1` does a release build and packs it with the licenses into `dist\mder0.1-windows-x64.exe`, a 7-Zip self-extractor (it needs 7-Zip installed). It finds CMake and Ninja from Visual Studio if they aren't on the PATH. Add `-Install C:\some\folder` to copy the result there too.

Pushing a tag like `v0.1.0` makes GitHub build and publish a release.

### macOS

You need Rust, Apple's command line tools, and CMake and Ninja (from Homebrew, for example). wxDragon builds wxWidgets on the first build. Then:

```
scripts/macos-app.sh
```

That builds mder and wraps it into `target/release/mder.app`, signed for this Mac only, and tells macOS about it. Add `--debug` for a debug build. mder.app needs macOS 12 or later.

### Linux

Tested on Debian 13 with the MATE desktop and Orca. You need Rust and these packages (Debian and Ubuntu names):

```
build-essential cmake ninja-build pkg-config libclang-dev libgtk-3-dev libwebkit2gtk-4.1-dev libssl-dev libpng-dev libjpeg-dev libtiff-dev libexpat1-dev libgl1-mesa-dev libglu1-mesa-dev libxkbcommon-dev
```

Then `cargo build --release`, and run `target/release/mder`. The first build takes a few minutes while it builds wxWidgets. `scripts/linux-pack.sh` builds it and packs the download, `dist/mder0.1-linux-x64.tar.gz`.

## License

mder is licensed under either of

- the [MIT license](LICENSE-MIT.txt), or
- the [Apache License, Version 2.0](LICENSE-APACHE.txt)

at your option. Please keep the credits in [NOTICE.txt](NOTICE.txt) when you use mder or parts of it.

Unless you say otherwise, any contribution you submit for inclusion in mder is licensed as above, without any additional terms or conditions.
