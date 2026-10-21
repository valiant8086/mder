"use strict";
// Runs inside the render pane. Talks to the app through window.mder.postMessage
// (a wxWebView script message handler) and is driven with run_script.

const post = (msg) => { try { window.mder.postMessage(msg); } catch (e) { /* not ready */ } };
const doc = document.getElementById("doc");

// The block for a source line: the last one starting at or before it.
function blockAt(line) {
  let best = null, bestLine = -1;
  for (const el of doc.querySelectorAll("[data-line]")) {
    const l = Number(el.dataset.line);
    if (l <= line && l >= bestLine) { best = el; bestLine = l; }
  }
  return best;
}

// Focusable only while focused, so NVDA doesn't treat blocks as controls.
function focusTemporarily(el) {
  if (!el.hasAttribute("tabindex")) {
    el.tabIndex = -1;
    el.addEventListener("blur", () => el.removeAttribute("tabindex"), { once: true });
  }
  el.focus();
}

// Linux, where the screen reader is Orca. It follows the text caret in
// browse mode, and reads focus differently from NVDA, so the NVDA tricks
// below would only confuse it.
const orca = /Linux/.test(navigator.platform);

// WebKitGTK reports a list inside a list item wrongly: the item's text runs
// on into the inner list's, and the inner list is reported separately too,
// which stops Orca's arrows at the list. An inner list placed between the
// outer list's items reports cleanly, so for Orca, inner lists move there,
// just after their item. Browsers accept that shape, though it isn't
// strictly valid HTML.
function hoistNestedLists(root) {
  for (const item of root.querySelectorAll("li")) {
    let after = item;
    for (const child of [...item.children]) {
      if (child.tagName === "UL" || child.tagName === "OL") {
        after.after(child);
        after = child;
      }
    }
  }
}
// WebKitGTK reports each bullet and number as a separate object that reads
// as nothing, so Orca never says "bullet". For Orca, they're written into
// each item as real text instead (drawing them with the style sheet threw
// WebKitGTK's line ends off by their length), and WebKit's own are hidden.
// Hiding them can make WebKit stop calling it a list, hence role="list".
const BULLETS = ["• ", "◦ ", "▪ "];
function writeMarkers(root) {
  for (const list of root.querySelectorAll("ul, ol")) {
    list.classList.add("mder-markers");
    list.setAttribute("role", "list");
    let depth = 0;
    for (let p = list.parentElement; p && p !== root; p = p.parentElement) {
      if (p.tagName === "UL" || p.tagName === "OL") depth++;
    }
    let number = Number(list.getAttribute("start") || 1);
    for (const item of list.children) {
      if (item.tagName !== "LI" || item.querySelector(":scope > .mder-marker")) continue;
      // Task list items show their checkbox instead, as on GitHub.
      if (item.querySelector(":scope > input[type=checkbox], :scope > .li-text > input[type=checkbox]:first-child")) continue;
      const marker = document.createElement("span");
      marker.className = "mder-marker";
      marker.textContent = list.tagName === "OL" ? `${number++}. ` : BULLETS[depth % BULLETS.length];
      item.prepend(marker);
    }
  }
}

if (orca) {
  hoistNestedLists(doc);
  writeMarkers(doc);
}
// How long the app's own focus news gets to reach Orca first. See mderFocusLine.
const ORCA_SETTLE_MS = 150;

// An empty element that can be focused just before the target. NVDA ignores
// focus coming back to the element it last saw focused, so focusing this
// first makes the move to the target always look new. It's made once, at
// load: changing the page right before a focus move makes NVDA lose it.
// Not for Orca: to it, a focused empty element looks like a control, which
// can flip it out of browse mode.
const sentinel = orca ? null : document.createElement("span");
if (sentinel) {
  sentinel.id = "mder-sentinel";
  sentinel.tabIndex = -1;
  document.body.appendChild(sentinel);
}

// How long a block keeps focus for Orca to take it in and read it, before
// letting go. See moveReaderTo.
const ORCA_READ_MS = 300;

// Called by the app to put the screen reader on this line.
window.mderFocusLine = (line) => {
  const block = blockAt(line) || doc.querySelector("[data-line]");
  if (!block) return "empty";
  return moveReaderTo(block);
};

// Called by the app when a menu closes over the rendered view. GTK gives
// focus back to the web view itself, not to anything in the page, so Orca
// isn't in the document any more. Put it back where it was reading.
window.mderRestoreReader = () => {
  if (!orca) return "not needed";
  // The menu reports closing before GTK has given focus back, so wait for
  // that to happen first, or Orca hears the web view last.
  setTimeout(() => {
    const at = elementAtCaret();
    const block = (at && at.closest("#doc [data-line]")) || doc.querySelector("[data-line]");
    if (block) moveReaderTo(block);
  }, ORCA_RESTORE_MS);
  return "restoring shortly";
};

// How long after a menu closes before Orca is put back. See mderRestoreReader.
const ORCA_RESTORE_MS = 250;

// A placement waiting for the page to get focus. See moveReaderTo.
let pendingPlace = null;

// Puts the screen reader on a block of the page, and returns what happened,
// for mder's debug log.
function moveReaderTo(block) {
  const where = block.dataset.line || block.id;
  if (orca) {
    // Orca only uses browse mode once something inside the page has gained
    // focus. So the block gains focus, and keeps it while Orca reads it:
    // letting go at once, WebKitGTK sometimes reports only the letting go,
    // and Orca never enters the page; Orca also ignores a focus that's gone
    // by the time it looks. Then it lets go, since left focused, some
    // blocks put Orca in focus mode, where the arrows don't read; and the
    // caret moves there, for Enter and F6. Orca has to hear about the block
    // after the app's own view gets focus, or it ends up outside the page.
    // The page and the app tell Orca separately, so when the app has just
    // moved focus here, wait for the page to get it, then a moment more.
    // Orca reads the block from its focus; the caret only marks the place,
    // for Enter and F6, since Orca never speaks a caret a page moves.
    const place = () => {
      focusTemporarily(block);
      setTimeout(() => {
        block.blur();
        window.getSelection().collapse(block, 0);
      }, ORCA_READ_MS);
    };
    if (document.hasFocus()) {
      place();
      return `${where}, moved within the page`;
    }
    // Only the latest placement waits: a menu command such as Switch pane
    // can queue one up just before F6 queues another.
    if (pendingPlace) window.removeEventListener("focus", pendingPlace);
    pendingPlace = () => {
      pendingPlace = null;
      setTimeout(place, ORCA_SETTLE_MS);
    };
    window.addEventListener("focus", pendingPlace, { once: true });
    return `${where}, placing when the page gets focus`;
  }
  // A focusable list item puts NVDA in focus mode, so NVDA gets the item's
  // own text instead.
  const target = block.tagName === "LI" ? (block.querySelector(":scope > .li-text") || block) : block;
  sentinel.focus();
  focusTemporarily(target);
  // Did the page really take focus?
  return `${where}, page has focus: ${document.hasFocus()}, focused: ${document.activeElement === target}`;
}

// Called by the app after an edit: patch the DOM in place so NVDA keeps its place.
window.mderUpdate = (html) => {
  const next = document.createElement("div");
  next.innerHTML = html;
  if (orca) {
    hoistNestedLists(next);
    writeMarkers(next);
  }
  morph(doc, next);
  return "ok";
};

function morph(cur, next) {
  const a = cur.childNodes, b = next.childNodes;
  for (let i = 0; i < b.length; i++) {
    const n = b[i], c = a[i];
    if (!c) { cur.appendChild(n.cloneNode(true)); continue; }
    if (c.nodeType !== n.nodeType || c.nodeName !== n.nodeName) { cur.replaceChild(n.cloneNode(true), c); continue; }
    if (c.nodeType === Node.ELEMENT_NODE) { syncAttributes(c, n); morph(c, n); }
    else if (c.data !== n.data) c.data = n.data;
  }
  while (a.length > b.length) cur.removeChild(cur.lastChild);
}

function syncAttributes(c, n) {
  for (const { name } of [...c.attributes]) {
    if (name !== "tabindex" && !n.hasAttribute(name)) c.removeAttribute(name);
  }
  for (const { name, value } of n.attributes) {
    if (c.getAttribute(name) !== value) c.setAttribute(name, value);
  }
}

// Enter in browse mode clicks the block under NVDA's cursor: edit that line.
document.addEventListener("click", (e) => {
  const link = e.target.closest("a[href]");
  if (link) {
    // A link within the document moves the screen reader to its target:
    // scrolling there, as the page would by itself, doesn't move it. Other
    // links would take the pane off this document, so they do nothing yet.
    e.preventDefault();
    const href = link.getAttribute("href");
    if (href.startsWith("#")) {
      let id = href.slice(1);
      try { id = decodeURIComponent(id); } catch (_) { /* use it as written */ }
      const target = document.getElementById(id);
      if (target) moveReaderTo(target);
    }
    return;
  }
  if (e.target.closest("input, button")) return;
  const block = e.target.closest("#doc [data-line]");
  if (block) { e.preventDefault(); post("goto:" + block.dataset.line); }
});

// The element under the screen reader's cursor: where the selection starts,
// since Orca moves the caret as you read, or else the focused element.
// Focus alone isn't enough: after Tab to a link, Orca can read on elsewhere
// while focus stays on the link.
function elementAtCaret() {
  const sel = window.getSelection();
  let node = sel && sel.rangeCount ? sel.anchorNode : null;
  if (!node || !doc.contains(node)) node = document.activeElement;
  return node && (node.nodeType === Node.ELEMENT_NODE ? node : node.parentElement);
}

// The line where the page's text caret is, if it's in the document. Orca
// moves the caret as you read, so this is where you're reading.
function caretLine() {
  const sel = window.getSelection();
  const node = sel && sel.rangeCount ? sel.anchorNode : null;
  if (!node || !doc.contains(node)) return "";
  const el = node.nodeType === Node.ELEMENT_NODE ? node : node.parentElement;
  const block = el && el.closest("#doc [data-line]");
  return block ? block.dataset.line : "";
}

// The app's keys never reach it while the web view has focus: its menu
// shortcuts, and even Alt for the menu bar. Pass them on. On Linux, keys
// the page doesn't use go back to the window, which opens its own menus, so
// the menu keys are left alone there.
const forwardMenuKeys = !orca;
let altAlone = false;
document.addEventListener("keydown", (e) => {
  if (e.key === "Alt") {
    if (!e.repeat) altAlone = true;
    return;
  }
  altAlone = false;
  const plain = !e.ctrlKey && !e.altKey && !e.shiftKey;
  // F6 says where the caret is, for the app to decide what to do with it.
  if (e.key === "F6" && !e.ctrlKey && !e.altKey) { e.preventDefault(); post("switch:" + caretLine()); }
  else if (e.key.toLowerCase() === "s" && e.ctrlKey && !e.altKey && !e.shiftKey) { e.preventDefault(); post("save"); }
  else if (e.key === "," && e.ctrlKey && !e.altKey && !e.shiftKey) { e.preventDefault(); post("options"); }
  else if (e.key === "F10" && plain && forwardMenuKeys) { e.preventDefault(); post("menu:"); }
  // Enter on a block edits its line. NVDA and VoiceOver turn Enter into a
  // click (handled above), and the page never sees the key; Orca only does
  // that for links and form fields, and passes Enter on elsewhere.
  // On a link, Enter is the link's.
  else if (e.key === "Enter" && plain) {
    const at = elementAtCaret();
    const block = at && !at.closest("a[href], input, button, textarea") && at.closest("#doc [data-line]");
    if (block) { e.preventDefault(); post("goto:" + block.dataset.line); }
  }
  // Alt plus a letter opens that menu. Ctrl+Alt is AltGr on some keyboards.
  else if (e.altKey && !e.ctrlKey && /^[a-z]$/i.test(e.key) && forwardMenuKeys) { e.preventDefault(); post("menu:" + e.key.toLowerCase()); }
}, true);
// Alt pressed and released on its own opens the menu bar.
document.addEventListener("keyup", (e) => {
  if (e.key === "Alt" && altAlone && forwardMenuKeys) { e.preventDefault(); post("menu:"); }
  altAlone = false;
}, true);
window.addEventListener("blur", () => { altAlone = false; });
