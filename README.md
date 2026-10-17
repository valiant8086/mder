# mder

A markdown editor and renderer built for screen readers, NVDA first.

The window has two panes: a plain text editor for the markdown source, and a rendered view you read in browse mode. They stay in sync:

- F6 switches panes. Going to the rendered view puts the screen reader on the block for the editor's caret line.
- Enter on a block in the rendered view takes you to that line in the editor.
- The rendered view updates as you type, patching only what changed so the screen reader keeps its place.
- If another program changes the file, mder reloads it. With unsaved edits, it asks first.

How the syncing works, and what we learned about NVDA in web views along the way, is in [docs/nvda-findings.md](docs/nvda-findings.md).

## Getting it

Download the zip from the [releases page](https://github.com/valiant8086/mder/releases), unzip it anywhere, and run `mder.exe`. It's a single portable exe; nothing gets installed. It needs the WebView2 runtime, which ships with Windows 10 and 11.

To add mder to Explorer's Open with menu for markdown files, run this once from wherever you keep it:

```
mder --register
```

It only adds mder as a choice; it doesn't change which program opens markdown files by default. It writes to your user's part of the registry, so it needs no admin rights. If you move mder, run it again. `mder --unregister` removes it.

## Command line

```
mder [--edit] [--line N] [--find TEXT] [--anchor SLUG] [FILE[:LINE][#SLUG]]
```

- `mder notes.md` opens the file.
- `mder notes.md:42` or `mder --line 42 notes.md` goes to line 42.
- `mder notes.md#setup-steps` or `mder --anchor setup-steps notes.md` goes to the heading with that anchor (GitHub style: lowercase, punctuation dropped, spaces as hyphens).
- `mder --find "some text" notes.md` goes to the first line containing that text. An exact match wins, otherwise the match ignores case.
- `--edit` lands in the editor instead of the rendered view.

Each file opens in one window. Running mder again for a file that's already open sends the location to that window, brings it to the front and exits with code 0. The window reloads the file from disk first. Exit code 1 means the open window didn't respond.

This makes mder easy to drive from another program. For example, after changing a file, run `mder --find "the changed text" file.md` to show it to the reader.

Set `MDER_LOG` to a file path to get a debug log.

## Building

You need Rust, and on Windows the Visual Studio C++ build tools with CMake and Ninja (both come with Visual Studio's C++ workload). wxDragon downloads prebuilt wxWidgets on the first build.

```
cargo build --release
```

`scripts\dist.ps1` does a release build and packs it into `dist\` with the licenses. It finds CMake and Ninja from Visual Studio if they aren't on the PATH. Add `-Install C:\some\folder` to copy the result there too.

Pushing a tag like `v0.1.0` makes GitHub build and publish a release.

### Other platforms

mder is built on wxWidgets, which also runs on Linux (with WebKitGTK) and macOS, so it should build there. Nobody has tried yet, and `--register` is Windows only. Reports and fixes are welcome.

## License

mder is licensed under either of

- the [MIT license](LICENSE-MIT.txt), or
- the [Apache License, Version 2.0](LICENSE-APACHE.txt)

at your option. Please keep the credits in [NOTICE.txt](NOTICE.txt) when you use mder or parts of it.

Unless you say otherwise, any contribution you submit for inclusion in mder is licensed as above, without any additional terms or conditions.
