(function () {
  var theme = "dark";
  try {
    var saved = window.localStorage.getItem("nexora.theme");
    if (saved === "light" || saved === "dark") {
      theme = saved;
    } else if (
      window.matchMedia &&
      window.matchMedia("(prefers-color-scheme: light)").matches
    ) {
      theme = "dark";
    }
  } catch (_err) {
    theme = "dark";
  }
  document.documentElement.dataset.theme = theme;
})();
