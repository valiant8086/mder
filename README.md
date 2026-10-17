# mder

A markdown editor and renderer built for screen readers, NVDA first.

The window has two panes: a plain text editor for the markdown source, and a rendered view you read in browse mode. They stay in sync:

- F6 switches panes. Going to the rendered view puts the screen reader on the block for the editor's caret line.
- Enter on a block in the rendered view takes you to that line in the editor.
- The rendered view updates as you type, patching only what changed so the screen reader keeps its place.
- If another program changes the file, mder reloads it. With unsaved edits, it asks first.

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

Needs Rust and, on Windows, the Visual Studio C++ tools with CMake and Ninja on the PATH. wxDragon downloads prebuilt wxWidgets on the first build.

```
cargo build --release
```

On Windows, the rendered view needs the WebView2 runtime, which ships with Windows 10 and 11.
