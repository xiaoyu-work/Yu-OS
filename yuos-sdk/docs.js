(function () {
  var STORAGE_KEY = "yuos_sdk_docs_lang";
  var VALID_LANGS = ["python", "node", "go", "rust"];

  function setLanguage(lang) {
    if (VALID_LANGS.indexOf(lang) === -1) lang = "python";
    document.body.setAttribute("data-active-lang", lang);
    try {
      localStorage.setItem(STORAGE_KEY, lang);
    } catch (_) {}
    var buttons = document.querySelectorAll(".lang-tab");
    for (var i = 0; i < buttons.length; i++) {
      var btn = buttons[i];
      var isActive = btn.getAttribute("data-select-lang") === lang;
      btn.classList.toggle("active", isActive);
      btn.setAttribute("aria-selected", isActive ? "true" : "false");
    }
  }

  document.addEventListener("DOMContentLoaded", function () {
    var saved = "python";
    try {
      var fromUrl = new URLSearchParams(window.location.search).get("lang");
      if (fromUrl && VALID_LANGS.indexOf(fromUrl) !== -1) {
        saved = fromUrl;
      } else {
        var stored = localStorage.getItem(STORAGE_KEY);
        if (stored && VALID_LANGS.indexOf(stored) !== -1) saved = stored;
      }
    } catch (_) {}

    setLanguage(saved);

    var buttons = document.querySelectorAll(".lang-tab");
    for (var i = 0; i < buttons.length; i++) {
      buttons[i].addEventListener("click", function (e) {
        var lang = e.currentTarget.getAttribute("data-select-lang");
        setLanguage(lang);
      });
    }
  });
})();
