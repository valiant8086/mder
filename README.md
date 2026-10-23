# mder

A markdown editor and renderer built for screen readers, NVDA first.

What's with the name? MD-er? uh I dunno, but the md is definitely honoring the .md extension. It's also fast to speak and easy if silly to say. I say it as a word rather than spelling out M D, because it's funnier that way.

mder is written largely through agentic coding, with an AI coding agent doing most of the programming. See [How mder is made](#how-mder-is-made).

The window has two panes: a plain text editor for the markdown source, and a rendered view you read in browse mode. They stay in sync:

- F6 switches panes. It's a toggle.
  - Going to the rendered view puts the screen reader on the block for the editor's caret line.
  - Going back to the editor at the place you're reading, however, depends on the system.
    - Windows: press Enter in the rendered view. mder detects the screen reader's click there, switches to the editor, and moves the caret to that position. Clicking with a mouse in the rendered view works too.
    - Mac: similar to Windows, except that VoiceOver users press VO+Space, or with Quick Nav on, Up and Down Arrow together, to issue the click that makes mder switch to the editor and move the caret.
    - Linux: Orca moves the rendered view's own text cursor as you read, so mder always knows where Orca is reading. Thus, plain F6 works both ways, keeping the editor and the rendered view in step.
  - So when not on Linux, if you read ahead in the rendered view and press F6, the editor is still where you left it, which causes the next press of F6 to move back to that line in the rendered view. Something to be mindful of.
- Links in the rendered view only work within the document, such as links to headings. Links to websites and other files don't open anything yet.
- The rendered view updates as you type, patching only what changed so the screen reader keeps its place.

How the syncing works, and what we learned along the way, is written up for each screen reader:

- [NVDA](docs/nvda-findings.md): what a web page can and can't tell about where NVDA is, and how to move it from code.
- [Orca](docs/orca-findings.md): Orca needed different tricks from NVDA, and WebKitGTK has bugs with lists that mder works around. Orca 48 has two small gaps that Orca 50 doesn't, such as not announcing entering a list.
- [VoiceOver](docs/voiceover-findings.md): the rendered view works much as with NVDA; most of the work was around it, such as coming to the front. Tab skips links in the rendered view, as in Safari, so use VO+arrows or Quick Nav to reach them.

If another program changes a file that mder has open, it reloads both panes within about a second. If you have edits in mder that aren't saved yet, it asks first whether to reload the file, losing your edits, or to keep your edits.

## Getting it

Download the latest from the GitHub [releases page](https://github.com/valiant8086/mder/releases/latest). The file names start with the version, such as `mder0.1.5`, then say which system and processor they're for.

- On Windows, mder ships as a 7-Zip self-extracting exe (an SFX), such as `mder0.1.5-windows-x64.exe`, or `-arm64` for Windows on ARM. mder is a portable exe; nothing gets installed, though it writes to the registry if you choose to add it to Explorer's Open with menu (see [Open with](#open-with)).
  - Make a folder just for mder, such as `C:\Tools\mder`. The files unpack straight into the folder you choose, not into a folder of their own.
  - The easy way: copy the SFX into that folder and run it there. It offers its own folder, so just press Enter. You can delete the SFX afterwards.
  - Or run it from wherever you downloaded it, and choose mder's folder when it asks.
  - Then run `mder.exe` in that folder.
  - mder needs the WebView2 runtime, which comes with Windows 11, and with Windows 10 through Windows Update.
- On Mac, mder is a universal build that works on both Intel and Apple silicon Macs, and ships as a zip, `mder0.1.5-mac-universal.zip`. That should be familiar territory: inside it is mder.app. Move it where you want to keep it, such as Applications, before opening it; otherwise macOS runs it from a hidden temporary copy. mder isn't signed by Apple, so the first time you open it, macOS refuses: go to System Settings, Privacy & Security, find the message saying mder was blocked, and choose "Open Anyway". After that it opens normally. It needs macOS 12 or later.
- On Linux, mder ships as a tarball, such as `mder0.1.5-linux-x64.tar.gz` for most PCs, or `-arm64` for ARM machines such as a Raspberry Pi running a 64-bit system. Unpack it with your file manager (usually Extract Here in its context menu), or in a terminal with `tar -xzf mder0.1.5-linux-x64.tar.gz`. Either way you get a folder named `mder`; move it where you want to keep it, such as your home folder, and run the `mder` program inside it. mder uses your system's GTK 3 and WebKitGTK 4.1, which most desktops already have. If not, on Debian or Ubuntu, install them with `sudo apt install libgtk-3-0t64 libwebkit2gtk-4.1-0` (on releases before 2024, the first package is called `libgtk-3-0`). Tested on Debian 13 with MATE and Orca 48, and on Ubuntu 26.04 with GNOME and Orca 50.

To offer mder when you open markdown files from Explorer (or your Linux file manager), use Tools, Options, or run `mder --register` once. See [Open with](#open-with) below.

## Command line

```
mder [--edit | --view] [--line N] [--find TEXT] [--anchor SLUG] [--log FILE] [FILE[:LINE][#SLUG]]
mder --register
mder --unregister
```

On a Mac, start mder through `open`, with full paths. For example, instead of `mder --line 42 notes.md`, run:

```
open -n /Applications/mder.app --args --line 42 /path/to/notes.md
```

There's more about why below the table.

| Command | What it does |
| --- | --- |
| `mder notes.md` | Opens the file. |
| `mder notes.md:42` or `mder --line 42 notes.md` | Goes to line 42. |
| `mder notes.md#setup-steps` or `mder --anchor setup-steps notes.md` | Goes to the heading with that anchor (GitHub style: lowercase, punctuation dropped, spaces as hyphens). |
| `mder --find "some text" notes.md` | Goes to the first line containing that text. An exact match wins; otherwise the match ignores case. |
| `--edit` or `--view` | Opens in the editor, or in the rendered view, whatever the settings say. See [Which pane a file opens in](#which-pane-a-file-opens-in). |
| `--log FILE` | Writes a debug log to that file, for troubleshooting. See [Debug log](#debug-log). |
| `mder --register` | Windows and Linux: offers mder in Open with for every file type it supports, without changing the default app. Mac: makes mder the default app for those types. See [Open with](#open-with). |
| `mder --unregister` | Windows and Linux: removes mder from Open with. Mac: gives each type back to the app that opened it before mder; for any type where mder doesn't know which app that was, it says so, and you choose one in Settings. |

Each file opens in one window. Running mder again for a file that's already open in another instance of it sends the location (if provided) to that window, brings it to the front and exits with code 0. The window reloads the file from disk first. Exit code 1 means the open window didn't respond.

This makes mder easy to drive from another program or AI coding agent. For example, after changing a file, Claude could run `mder --find "the changed text" file.md` to show you the exact context in the rendered view. Just pressing Enter is all it takes to then make your own edits.

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

Settings are saved in `mder.ini`, next to mder.exe, so mder stays portable. If that folder can't be written to, they go in `%APPDATA%\mder` instead, or `~/.config/mder` on Linux. On macOS, Settings is in the mder menu (Command+Comma), and the file is in `~/Library/Application Support/mder`.

### Debug log

`mder --log C:\path\mder.log notes.md` makes that window add a line to the log file whenever it gets a location to go to, moves the editor's caret, puts the screen reader on a block in the rendered view, or notices the file changed on disk. Each line starts with mder's process number, so lines from different windows can be told apart. Without `--log`, mder writes no log at all.

If the file is already open, the window that has it starts logging to that file too, if it wasn't logging already. An Options setting is planned that will turn the log on, and choose its file, for every launch. When it arrives, `--log` will still override it for that launch.

### Open with

On Windows, registering adds mder to the list in the "How do you want to open this file?" dialog. In Explorer, open a markdown file's context menu, choose Open with, then "Choose another app", and mder is in that list. mder never changes which program opens a file type by default. If you want markdown files to always open in mder, check "Always use this app" in that dialog.

- **Tools, Options**, on the Integration tab, has a checklist of file types, grouped by kind. Each box shows whether that type is registered right now. Change them and press OK or Apply.
- **`mder --register`** does the same for every type at once, and **`mder --unregister`** removes them all. Run it from wherever you keep mder.

Registering writes to your user's part of the registry, so it needs no admin rights. It records where mder is, so if you move mder, register it again from its new place. Tools, Options shows any type still pointing at the old copy, and checking it moves it to this one.

On macOS, mder.app is always in Finder's Open With menu for markdown files. The Integration tab in Settings instead chooses which types open in mder when you double-click them. Checking one makes mder the default and remembers the app it replaced; unchecking gives the type back to that app, or asks which app to use. macOS asks you to confirm each change.

On Linux, registering works as on Windows: mder joins the Open with list in your file manager, and never changes which program opens a file type by default. Linux desktops go by file type rather than extension, so the Integration tab has one box for all markdown. Registering writes `mder.desktop` to `~/.local/share/applications`, and also teaches your desktop that .mdown, .mkdn and .mdwn are markdown (some systems call them plain text), with `mder.xml` in `~/.local/share/mime/packages`. Unchecking the box removes both. If no default program is set for markdown, the desktop may open it in mder on double-click, since mder names markdown more closely than general text editors do; choose a default in your file manager's Open with dialog to change that. `--register` and `--unregister` work here too.

A note on portability: registering is the one part of mder that isn't portable, since that is recorded on the computer instead of in mder's folder. If you move mder, register it again from its new place. If you're removing mder from a computer, unregister it first, so it isn't offered anymore.

## Updates

mder checks for a new version when it launches, no more than once a day. If there is one, it shows what changed since your version, with three choices: Install Now, Ignore Version, and Maybe Later. Ignore Version skips that version and avoids prompting you to update until the next version is out.
You can also check any time with Help > Check for update, and turn the daily check off in Tools > Options on the General tab (mder > Settings on the Mac).

Install Now downloads the new version, puts it in place of the old one, and restarts mder on the same file, in the same pane. Every other open mder window restarts on the new version too; one with unsaved changes asks first whether to save them. mder replaces itself in its own folder, so that needs to be a folder you can write to; if it isn't, mder offers to open the download page instead. Updates come from mder's GitHub releases. On the Mac, updating should not cause mder to be blocked from running again.

## Building

You need Rust, and on Windows the Visual Studio C++ build tools with CMake and Ninja (both come with Visual Studio's C++ workload). wxDragon downloads wxWidgets' source and builds it on the first build, which takes a few minutes.

```
cargo build --release
```

`scripts\dist.ps1` does a release build and packs it with the licenses into `dist\mder0.1.5-windows-x64.exe`, a 7-Zip self-extractor (it needs 7-Zip installed). It finds CMake and Ninja from Visual Studio if they aren't on the PATH. Add `-Install C:\some\folder` to copy the result there too.

Pushing a tag like `v0.1.0` makes GitHub build and publish a release.

If you send a change, please leave the version in Cargo.toml as it is. Its last number counts the maintainer's builds, and goes up when they build your change.

### macOS

You need Rust, Apple's command line tools, and CMake and Ninja (from Homebrew, for example). wxDragon builds wxWidgets on the first build. Then:

```
scripts/macos-app.sh
```

That builds mder and wraps it into `target/release/mder.app`, signed for this Mac only, and tells macOS about it. Add `--debug` for a debug build. mder.app needs macOS 12 or later.

### Linux

Tested on Debian 13 with the MATE desktop (X11) and Ubuntu 26.04 with the GNOME desktop (Wayland), Orca on both. You need Rust and these packages (Debian and Ubuntu names):

```
build-essential cmake ninja-build pkg-config libclang-dev libgtk-3-dev libwebkit2gtk-4.1-dev libssl-dev libpng-dev libjpeg-dev libtiff-dev libexpat1-dev libgl1-mesa-dev libglu1-mesa-dev libxkbcommon-dev
```

Then `cargo build --release`, and run `target/release/mder`. The first build takes a few minutes while it builds wxWidgets. `scripts/linux-pack.sh` builds it and packs the download, `dist/mder0.1.5-linux-x64.tar.gz`.

## How mder is made

mder is built with heavy use of agentic coding. An AI coding agent, Claude Code, writes most of the code, scripts and documentation, working from my direction. I decide what mder does and how it should behave, and I test the changes myself with screen readers: NVDA, JAWS and Narrator on Windows, VoiceOver on the Mac, and Orca on Linux. What we learned along the way, such as how NVDA reads web views, is written up in the docs folder.

## License

mder is licensed under either of

- the [MIT license](LICENSE-MIT.txt), or
- the [Apache License, Version 2.0](LICENSE-APACHE.txt)

at your option. Please keep the credits in [NOTICE.txt](NOTICE.txt) when you use mder or parts of it.

Unless you say otherwise, any contribution you submit for inclusion in mder is licensed as above, without any additional terms or conditions.
