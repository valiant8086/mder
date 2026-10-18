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

// An empty element that can be focused just before the target. NVDA ignores
// focus coming back to the element it last saw focused, so focusing this
// first makes the move to the target always look new. It's made once, at
// load: changing the page right before a focus move makes NVDA lose it.
const sentinel = document.createElement("span");
sentinel.id = "mder-sentinel";
sentinel.tabIndex = -1;
document.body.appendChild(sentinel);

// Called by the app to put the screen reader on this line.
window.mderFocusLine = (line) => {
  const block = blockAt(line) || doc.querySelector("[data-line]");
  if (!block) return "empty";
  const target = block.tagName === "LI" ? (block.querySelector(":scope > .li-text") || block) : block;
  sentinel.focus();
  focusTemporarily(target);
  // For mder's debug log: did the page really take focus?
  return `${block.dataset.line}, page has focus: ${document.hasFocus()}, focused: ${document.activeElement === target}`;
};

// Called by the app after an edit: patch the DOM in place so NVDA keeps its place.
window.mderUpdate = (html) => {
  const next = document.createElement("div");
  next.innerHTML = html;
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
    // Keep the pane on this document. In-page links still work.
    if (!link.getAttribute("href").startsWith("#")) e.preventDefault();
    return;
  }
  if (e.target.closest("input, button")) return;
  const block = e.target.closest("#doc [data-line]");
  if (block) { e.preventDefault(); post("goto:" + block.dataset.line); }
});

// The app's keys never reach it while the web view has focus: its menu
// shortcuts, and even Alt for the menu bar. Pass them on.
let altAlone = false;
document.addEventListener("keydown", (e) => {
  if (e.key === "Alt") {
    if (!e.repeat) altAlone = true;
    return;
  }
  altAlone = false;
  const plain = !e.ctrlKey && !e.altKey && !e.shiftKey;
  if (e.key === "F6" && !e.ctrlKey && !e.altKey) { e.preventDefault(); post("switch"); }
  else if (e.key.toLowerCase() === "s" && e.ctrlKey && !e.altKey && !e.shiftKey) { e.preventDefault(); post("save"); }
  else if (e.key === "F10" && plain) { e.preventDefault(); post("menu:"); }
  // Alt plus a letter opens that menu. Ctrl+Alt is AltGr on some keyboards.
  else if (e.altKey && !e.ctrlKey && /^[a-z]$/i.test(e.key)) { e.preventDefault(); post("menu:" + e.key.toLowerCase()); }
}, true);
// Alt pressed and released on its own opens the menu bar.
document.addEventListener("keyup", (e) => {
  if (e.key === "Alt" && altAlone) { e.preventDefault(); post("menu:"); }
  altAlone = false;
}, true);
window.addEventListener("blur", () => { altAlone = false; });
