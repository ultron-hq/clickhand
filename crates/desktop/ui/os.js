// Tags the page with its OS (<html data-os="macos|windows|linux">) so app.css can follow
// each OS's conventions, and gives key hints in that OS's style.
window.OS = /Mac/.test(navigator.userAgent) ? "macos" : /Windows/.test(navigator.userAgent) ? "windows" : "linux";
document.documentElement.dataset.os = window.OS;
window.KEYS = window.OS === "macos"
  ? { enter: "↩", esc: "esc", cancel: "⌘ .", up: "↑" }
  : { enter: "Enter", esc: "Esc", cancel: "Ctrl+.", up: "↑" };
