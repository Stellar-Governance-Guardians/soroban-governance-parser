// Initialize the vendored Mermaid build for <pre class="mermaid"> blocks.
// No network access: mermaid.min.js is served from this site.
(function () {
  function theme() {
    var dark = ['ayu', 'navy', 'coal'];
    var html = document.documentElement;
    return dark.some(function (name) {
      return html.classList.contains(name);
    })
      ? 'dark'
      : 'default';
  }

  function start() {
    if (typeof mermaid === 'undefined') {
      return;
    }
    mermaid.initialize({
      startOnLoad: true,
      securityLevel: 'strict',
      theme: theme(),
    });
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', start);
  } else {
    start();
  }
})();
