# Keeping an editor in sync with Orca in a web view

On Linux, mder shows markdown source in a GTK text view and the rendered result in a WebKitGTK web view, both through wxWidgets. As with NVDA (see [nvda-findings.md](nvda-findings.md)), the hard part is moving Orca into the page from code, and knowing where it is. Orca turned out to need different tricks from NVDA, and WebKitGTK has a few bugs that get in Orca's way.

Tested with Orca 48.1 on Debian 13 (MATE on X11, WebKitGTK 2.54), and Orca 50.2 on Ubuntu 26.04 (GNOME on Wayland, WebKitGTK 2.52). The accessibility tree was read over AT-SPI while testing, which is how most of these were found.

## Getting Orca into the page

- **Focus WebKit's own view, not wxWidgets' wrapper.** wxWidgets puts the web view inside a scrolled window and focuses that. Then keys never reach the page, and Orca calls it "scroll pane" and stays outside. mder gives focus to WebKit's view itself, with GTK, and only that: going through the wrapper first makes Orca say "scroll pane" again.
- **Orca only uses browse mode once something inside the page has gained focus,** and only if it hears about that after the web view itself got focus. The app and the page report focus separately, so the order is a race. mder waits for the page's own focus event, then 150 milliseconds more, then focuses the block for the editor's line.
- **Keep the block focused for a moment.** Focusing a block and letting go at once, as works for NVDA, failed about three times in four: WebKitGTK sometimes merged the two and reported only the letting go, and Orca ignores a focus that's already gone by the time it looks. mder keeps the block focused for 300 milliseconds while Orca reads it.
- **Then let go.** Left focused, some blocks put Orca in focus mode, where the arrows don't read. mder blurs the block and puts the page's text cursor there instead.
- **Skip the NVDA tricks.** The empty placeholder element mder focuses first for NVDA, and the focusable text at the start of a list item, looked like nameless controls to Orca, and browse mode stopped working after a round trip or two. On Linux, mder leaves both out.
- **Moving the text cursor from script is never spoken.** Orca's web support deliberately doesn't announce a caret move the page makes itself, so all the speech on arrival has to come from the focus above.

## Finding where Orca is

- **Orca moves the page's real text cursor as you read in browse mode.** NVDA and VoiceOver keep their reading position to themselves, but with Orca, the page's selection follows you. So the page can tell which block you're on at any time. mder sends that line with F6, and on Linux the editor follows, so F6 keeps both panes in step both ways.
- **Enter isn't a click.** NVDA and VoiceOver turn Enter into a click on any block; Orca only does that for links and controls, and passes Enter on as a key elsewhere. mder's page handles the key itself, using the block at the text cursor, so Enter takes you to that line in the editor as on Windows.

## WebKitGTK bugs, and what mder does about them

- **Lists inside list items stop Orca.** With a list nested in an item, WebKitGTK reports the item's text running on into the inner list's text, and reports the inner list as its own object as well. Orca can't match the text to the objects: the arrows stop at the list from above and below, and H jumps over it. Six shapes of nested list all failed, so it isn't mder's HTML. On Linux, mder moves each inner list out of its item, to just after it within the outer list, which WebKitGTK reports cleanly. Orca still says "nesting level 1" for the inner list.
- **Bullets and numbers read as nothing.** WebKitGTK reports each one as a separate object with no text. Drawing them with the style sheet put them into the text instead, but threw WebKitGTK's line ends off by their length, cutting the end off every item. mder writes "• ", "◦ " or the number into each item as real text, hides WebKit's own, and marks the lists `role="list"`, since hiding the markers otherwise stops WebKit calling them lists. Task list items, with their checkboxes, get no bullet.
- **Live regions say nothing.** An `aria-live` region produced no events from WebKitGTK at all, so Orca never spoke it. mder doesn't rely on them.
- **Editable regions get flattened.** In a quick test of `contenteditable` content, for a possible future editing view, WebKitGTK reported it as flat text unless the region had `role="article"`. mder doesn't use editable regions yet.

These are worth reporting to WebKitGTK; the first two especially.

## The desktop around the page

- **Closing a menu leaves Orca outside the page.** GTK gives focus back to WebKit's view, not to anything inside the page, so Orca says "grayed" and stops. mder puts Orca back on the block at the text cursor a quarter second after a menu closes, once GTK has finished.
- **Menu keys belong to GTK.** On Windows, mder's page forwards F10 and Alt+letter to open menus. On Linux the page leaves them alone and GTK opens its own menus. Alt alone doesn't open the menu bar, which is normal for GTK apps.
- **Bringing a window forward.** On X11, window managers refuse a plain raise from an app in the background, which matters when a second launch hands a file to the window that has it. mder asks the way a taskbar does, with a `_NET_ACTIVE_WINDOW` request marked as coming from a pager, which they honor. On Wayland, a window can only come forward with an activation token: the file manager gives one to each launch when the app's desktop entry says `StartupNotify=true`, and mder passes it to the window that has the file. A launch from a terminal may get no token, so the window may not come forward.
- **Keeping track of the pane.** By the time GTK reports the window going to the background, it has already taken focus from both panes. And when mder focuses WebKit's view directly, wxWidgets doesn't see focus leave the editor, so the editor gets no focus event when it comes back. mder keeps note of the pane itself whenever focus moves.

## wxWidgets controls on GTK

- **A tree list's labels are hidden from Orca.** wxTreeListCtrl draws its checkbox labels with a custom renderer that GTK doesn't expose, so Orca hears only "collapsed tree level 1". mder uses plain checkboxes on Linux instead.
- **A radio box's title goes to the next control too.** wxWidgets on GTK makes each radio box's title the label of the next focusable control in its parent, so the second of two radio groups was read with both titles. Giving each radio box a panel of its own avoids it.
- **Switching tabs in a dialog:** focus moved into a newly shown tab page straight away gets lost while GTK is still showing the page, leaving focus on the tabs. mder moves it 50 milliseconds later.

## Orca 48 compared with Orca 50

Otherwise the two read the rendered view the same. Two gaps on Debian's Orca 48 are gone in Orca 50:

- Orca 48 doesn't announce entering a list at the top level (nested lists are announced). Orca 50 does.
- In Orca 48, pressing F6 to the editor and straight back, without moving, says "grayed" and nothing else, though Orca is in the right place and browse mode works. Orca 50 reads the current line.

Arriving in the rendered view with Orca 48 starts with "grayed": that's Orca describing WebKit's view on the way in.

## Not tested yet

- Other desktops, such as KDE, and Orca on Wayland under Debian.
- Whether newer WebKitGTK versions fix the list bugs. The Ubuntu machine didn't answer this: its WebKitGTK (2.52) is older than Debian's (2.54), and mder always applies its list workarounds on Linux, so Orca there only ever heard the workarounds. It needs a plain test page, without them, on a newer WebKitGTK.
