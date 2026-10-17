# Keeping an editor in sync with NVDA in a web view

mder shows markdown source in a native edit control and the rendered result in a web view (WebView2 on Windows). The hard part is keeping them in sync: knowing where NVDA is in the rendered view, and moving it somewhere from code.

A web page can't see NVDA's browse mode cursor directly. These are the results of testing what a page *can* see and do. They were tested with NVDA 2025.3.2 in Edge 155, which uses the same engine as WebView2. The probe used is in [spikes/nvda-probe](../spikes/nvda-probe).

## Finding where NVDA is

- **Arrowing in browse mode tells the page almost nothing.** No focus or selection events fire. The page only sees scroll events as NVDA scrolls content into view, which is too rough to sync on.
- **Enter does tell the page.** Pressing Enter in browse mode on an ordinary block (a paragraph, list item, table cell or code block) sends a real click to that element, and NVDA stays in browse mode. The click's target is the block NVDA was on. mder uses this: Enter on a rendered block takes you to its source line.
- A whole code block reports as one element. Its click arrived at coordinates 0,0, through the element's default action, so the page can't tell which line inside the block NVDA was on.

## Moving NVDA

- **Calling `element.focus()` from script moves NVDA there** and reads the element, even when the user is reading somewhere else in the page.
- Plain blocks aren't focusable, so give the element `tabindex="-1"` just before focusing it, and remove it again on blur.
- **Don't make list items focusable.** NVDA treats a focusable `<li>` like an option in a list box: Enter switches to focus mode and no click arrives. mder wraps a list item's leading text in a `<span>` and focuses that instead.

## Updating the page without losing NVDA's place

- **Replacing the whole document** makes NVDA rebuild its view and keep the same character offset. If content above the cursor changed length, the cursor ends up somewhere else.
- **Patching only the elements that changed** keeps NVDA on the right line, even when the changed text is above the cursor and gets longer. mder diffs the new HTML against the live page and updates it in place.

## Switching between the editor and the page

With the editor and the web view in separate native windows, NVDA switches cleanly: it leaves browse mode when focus moves to the edit control, and comes back to browse mode in the web view. Editors that put the source and the preview in one web document, such as VS Code, can leave NVDA stuck in browse mode instead.

## Not tested yet

- Other screen readers. JAWS and Narrator should follow `focus()` the same way, but nobody has checked how they handle Enter on a block.
- WebKit on Linux and macOS, with Orca and VoiceOver.
