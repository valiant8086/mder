# Keeping an editor in sync with VoiceOver in a web view

On the Mac, mder shows markdown source in an AppKit text view and the rendered result in a WebKit web view (WKWebView), both through wxWidgets. Most of what works for NVDA (see [nvda-findings.md](nvda-findings.md)) works for VoiceOver too. What took work on the Mac was everything around the page: coming to the front, keys in dialogs, and the text view itself.

Tested with VoiceOver on macOS 27.2, on Apple silicon.

## The page

- **Focusing a block from script moves VoiceOver there,** as it does NVDA, so F6 into the rendered view lands on the editor's line. mder uses the same method as for NVDA on the Mac.
- **VO+Space on a block is a click,** like NVDA's Enter, so it takes you to that line in the editor. With Quick Nav on, Up and Down Arrow together do the same.
- **The page can't see where VoiceOver is reading,** as with NVDA. So F6 from the rendered view leaves the editor where it was. (Orca is the exception: see [orca-findings.md](orca-findings.md).)
- **Tab skips links.** In a web view on the Mac, Tab goes from the page straight to the next control, as in Safari, while Windows and Linux stop on each link. Option+Tab moves between links. VoiceOver users reach them with VO+arrows, or L in Quick Nav.

## Running as a real app

- **A bare program started from Terminal isn't treated as an app.** Its window may never become active, the menu bar stays generic, and a Terminal window hangs around. mder is wrapped as mder.app, so it gets its own menus and comes to the front properly.
- **Finder doesn't pass files on the command line.** It sends the app an "open documents" message just after it starts. mder waits for that before building its window, so a file opened from Finder opens in the pane your settings choose.
- **About, Settings and Quit go in the app menu,** which wxWidgets does for its standard menu IDs, and there's no Tools menu on the Mac.

## Coming to the front

- **Since macOS 14, an app can only take the front if the active app lets it.** When a second launch hands a file to the mder window that already has it, that launch was the active app for about 50 milliseconds, then quit, and macOS gave the front back to Finder. The open window stayed behind, and VoiceOver said "mder is not responding", having turned to an app that had just vanished. Now the second launch waits until macOS has made it active, yields the front to the other mder, and stays until that mder has taken it. Yielding by mder's shared app identifier did nothing; yielding to the other mder's process works.
- **VoiceOver puts its cursor back where it was when an app comes forward,** unless focus has already moved. So on the Mac, mder moves focus first and then comes forward: the opposite order from Windows, where NVDA needs the window active first.

## Dialogs

- **Control+Tab never reaches a dialog.** macOS turns it into "move to the next control" before any control sees the key, so Control+Tab between tabs did nothing. mder watches for it as the app receives the key, while Settings is open. Command+Shift+] and Command+Shift+[ also switch tabs, as in Safari and Finder.
- **VoiceOver doesn't say "collapsed" on a collapsed tree branch.** It says "expanded" once it's open, but nothing before, so nothing shows there's anything inside. Finder's own list view behaves the same, so it's VoiceOver, not mder. mder adds a line after its tree: "Expand a file type to choose its extensions one by one."
- **System confirmations don't always take focus.** When mder changes the default app for a file type, macOS asks you to confirm in a dialog of its own, which VoiceOver may not move to. You can find it in VoiceOver's window chooser (VO+F1, twice). mder carries on meanwhile, and updates once you've answered.

## The editor

- **The Mac's text view changes what you type, by default.** Smart quotes and dashes, your text replacements, spelling autocorrection, and smart spacing around pasted words are all on in a standard Mac text view. In mder they turned straight quotes curly and double hyphens into dashes, in code too, and that was saved to the file. Nothing was announced. It was only noticed when the updater asked to save changes nobody had made. mder turns all of these off in its editor; spelling is still underlined.

## Not tested yet

- An Intel Mac. The download runs on both, but only Apple silicon was tested with VoiceOver; the Intel half has only been started under Rosetta, on GitHub's build machines.
- macOS versions before 27, down to 12, which mder supports.
