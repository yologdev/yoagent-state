(function () {
  function loadScript(src) {
    return new Promise(function (resolve, reject) {
      var script = document.createElement("script");
      script.src = src;
      script.async = true;
      script.onload = resolve;
      script.onerror = reject;
      document.head.appendChild(script);
    });
  }

  function renderMermaid() {
    var blocks = document.querySelectorAll("pre code.language-mermaid");
    if (!blocks.length || !window.mermaid) {
      return;
    }

    blocks.forEach(function (code) {
      var container = document.createElement("div");
      container.className = "mermaid";
      container.textContent = code.textContent;
      code.parentElement.replaceWith(container);
    });

    window.mermaid.initialize({
      startOnLoad: false,
      securityLevel: "strict",
      theme: "neutral",
      flowchart: {
        curve: "basis",
      },
    });
    window.mermaid.run({ querySelector: ".mermaid" });
  }

  function start() {
    loadScript("https://cdn.jsdelivr.net/npm/mermaid@10/dist/mermaid.min.js")
      .then(renderMermaid)
      .catch(function () {
        // Leave Mermaid source blocks visible if the CDN is unavailable.
      });
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", start);
  } else {
    start();
  }
})();
