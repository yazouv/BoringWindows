// Sélecteur de langue : renvoie vers la même page dans l'autre langue.
// Le français est à la racine du site, l'anglais dans /en/.
// Fichier identique dans docs/fr et docs/en (vérifié par la CI).
(function () {
  const lang = document.documentElement.lang || "fr";
  // `path_to_root` est défini par mdBook sur chaque page.
  const root = new URL(typeof path_to_root === "string" ? path_to_root : "./", location.href);
  let page = location.href.slice(root.href.length) || "index.html";
  page = page.split("#")[0].split("?")[0];
  const target = lang === "en"
    ? new URL("../" + page, root)
    : new URL("en/" + page, root);

  const bar = document.querySelector(".right-buttons");
  if (!bar) return;
  const link = document.createElement("a");
  link.className = "lang-switch";
  link.href = target.href;
  link.title = lang === "en" ? "Lire en français" : "Read in English";
  link.setAttribute("aria-label", link.title);
  link.textContent = lang === "en" ? "FR" : "EN";
  bar.prepend(link);
})();
