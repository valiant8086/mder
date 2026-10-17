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

// Called by the app when switching from the editor: put NVDA on this line.
window.mderFocusLine = (line) => {
  const block = blockAt(line) || doc.querySelector("[data-line]");
  if (!block) return "empty";
  const target = block.tagName === "LI" ? (block.querySelector(":scope > .li-text") || block) : block;
  focusTemporarily(target);
  return block.dataset.line;
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

// The app's menu shortcuts don't fire while the web view has focus.
document.addEventListener("keydown", (e) => {
  if (e.key === "F6" && !e.ctrlKey && !e.altKey) { e.preventDefault(); post("switch"); }
  else if (e.key.toLowerCase() === "s" && e.ctrlKey && !e.altKey && !e.shiftKey) { e.preventDefault(); post("save"); }
}, true);
