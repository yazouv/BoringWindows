# Changelog

## [0.3.0](https://github.com/yazouv/BoringWindows/compare/v0.2.1...v0.3.0) (2026-10-02)


### Nouveautés

* binaire macOS Intel et config.toml par défaut dans la langue du système ([c4e712b](https://github.com/yazouv/BoringWindows/commit/c4e712beeebc0ef7e75fdd49f7abc60e37297934))

## [0.2.1](https://github.com/yazouv/BoringWindows/compare/v0.2.0...v0.2.1) (2026-10-02)


### Corrections

* **i18n:** langue initialisée sans écraser un choix concurrent ; CI sautée sur les PR de release ([9f29781](https://github.com/yazouv/BoringWindows/commit/9f29781f8ed42ffc53d847b19d4e6bca7726a1e7))
* **security:** canal des hooks authentifié et secrets hors de la ligne de commande curl ([ff35c0a](https://github.com/yazouv/BoringWindows/commit/ff35c0abc98c9ec54509fae5f7a57f463631f202))


### Documentation

* CLAUDE.md (commandes, CI/CD, architecture) ([9ace3fd](https://github.com/yazouv/BoringWindows/commit/9ace3fd37352840613d5ecf59d309dcc2be5498b))

## [0.2.0](https://github.com/yazouv/BoringWindows/compare/v0.1.0...v0.2.0) (2026-10-01)


### Nouveautés

* **agenda:** CalDAV et secrets dans le Gestionnaire d'identifiants ([fdbddb1](https://github.com/yazouv/BoringWindows/commit/fdbddb146aae78b4e951ff01f9981e06fa7d8315))
* **claude:** conversations récentes (reprise) et consommation estimée depuis les transcripts ([18040c7](https://github.com/yazouv/BoringWindows/commit/18040c70d583d32349bafc8dd2548e6fbb5c8237))
* **claude:** rappel sonore tant qu'une session attend (remind_secs) ([b0047c2](https://github.com/yazouv/BoringWindows/commit/b0047c27038930dcf4cb381390539dd09c1c7986))
* **distribution:** installeur Inno Setup attaché aux releases et manifestes winget ([087c9ef](https://github.com/yazouv/BoringWindows/commit/087c9efdea5fb1a5be4fbecb0d889a5ab3d2ec48))
* **étagère:** garder des fichiers déposés sur l'île, réglages et doc ([a4042a7](https://github.com/yazouv/BoringWindows/commit/a4042a7197f4e64f3c0bf2a9e0e0b6d1820d94fe))
* **layouts:** vues .slint personnelles chargées à l'exécution (layout.view) ([bbeac40](https://github.com/yazouv/BoringWindows/commit/bbeac400f60aec8858d80bb5bdb94143e3f5426c))
* **minuteur:** module bw-timer, ligne dans l'île, réglages et doc ([c71df8a](https://github.com/yazouv/BoringWindows/commit/c71df8ad33c3c03dda1cd5ac71fbce6ddc4d39c6))
* **musique:** visualiseur audio optionnel (capture loopback WASAPI) ([a9bc00d](https://github.com/yazouv/BoringWindows/commit/a9bc00dd75622bcb49a222ae1009ed5edc46aeee))
* **plugins:** plugins WASM sandboxés (wasmi) qui publient du texte dans l'île ([b15c134](https://github.com/yazouv/BoringWindows/commit/b15c1341a08fc97a6cfcca226c9ad55d6ee7c7cb))
* **réglages:** lecteurs musique à cocher, tailles de l'île, ordre des modules ([07c9016](https://github.com/yazouv/BoringWindows/commit/07c901657a183ddcf20edc6cf5cb0bbbb053e327))
* **volume:** changements de volume du système dans l'île (module bw-volume) ([f3627d3](https://github.com/yazouv/BoringWindows/commit/f3627d346aaa39c236031c1d57ae728ab2326b8e))


### Corrections

* **claude:** fenêtre de consommation à l'heure exacte, calage sur l'heure de reset (reset_at) ([25eb81f](https://github.com/yazouv/BoringWindows/commit/25eb81fd8ad79ddf075ca1b7c2c87760bcc3937f))
* code mort hors Windows signalé par clippy (macOS/Linux) ([a0f3fb5](https://github.com/yazouv/BoringWindows/commit/a0f3fb55bbded924838a93e809069df573aecdb2))


### Documentation

* options du minuteur dans la référence de configuration ([d5c4ef7](https://github.com/yazouv/BoringWindows/commit/d5c4ef7614a9454159b05e7f8f71e99ac933e630))

## [0.1.0](https://github.com/yazouv/BoringWindows/compare/v0.0.1...v0.1.0) (2026-10-01)


### Nouveautés

* ajouter le fichier README.md ([257503a](https://github.com/yazouv/BoringWindows/commit/257503ac4b4b70abe88c5804b3d4f0e62ad3f825))
* **claude:** diagnostic `doctor` et journal du relais ([c3f3853](https://github.com/yazouv/BoringWindows/commit/c3f38539f7da89fc99a00256497338eed2ee4c5e))
* phase 0 — fondations de l'île ([f54e7f4](https://github.com/yazouv/BoringWindows/commit/f54e7f42ddc844669552ba9b08e8054d7e9ce9fa))
* phase 1 — intégration Claude Code ([cb792ea](https://github.com/yazouv/BoringWindows/commit/cb792eac846ae44e556dce77cce0ac84751e36a0))
* phase 2 — musique en cours ([17b54db](https://github.com/yazouv/BoringWindows/commit/17b54dbad52d8ab9240ef838c2aa71e542a54769))
* phase 3 — agenda (calendriers ICS) ([3afbfa1](https://github.com/yazouv/BoringWindows/commit/3afbfa138acd033c25ea39259b8bbf64894f5cc8))
* releases automatiques (release-please) et mise à jour de l'app ([6369425](https://github.com/yazouv/BoringWindows/commit/63694252674a001ac570241945bcd04c57febe73))


### Corrections

* **agenda:** cours qui s'enchaînent, salle affichée, libellé des jours ([46f57d7](https://github.com/yazouv/BoringWindows/commit/46f57d792b7c1af725afa24b42ef53a977d1d5be))
* brancher Win32 une fois la fenêtre native créée ([c21cad6](https://github.com/yazouv/BoringWindows/commit/c21cad6aa95a58ff624f35b8e4f35afc89fd4688))
* **claude:** Échap efface la question, son dès que Claude attend ([cacd872](https://github.com/yazouv/BoringWindows/commit/cacd872bb0f5a62130a29b896977fbd839d9b914))
* **claude:** questions au terminal, demandes réglées ailleurs effacées, son ([e2737fa](https://github.com/yazouv/BoringWindows/commit/e2737fa09fcefaa2a7ddb6d71f3c5a70a88e1d60))
* **doctor:** ne pas journaliser ses propres tests, juger sur le dernier appel ([326f4b9](https://github.com/yazouv/BoringWindows/commit/326f4b9e05cfff40ef115381577a8a23058be375))
* **doctor:** rendre SESSION public et juger sur le dernier appel ([a0ab34e](https://github.com/yazouv/BoringWindows/commit/a0ab34e7346e2fd71af8a9e312936debbc0e3515))
* **doctor:** tester avec Git Bash (pas WSL) et cmd sans échappement Rust ([7873e67](https://github.com/yazouv/BoringWindows/commit/7873e674f4f581a440b34c58bb1360ac675330e0))
* **media:** lint clippy as_chunks (Rust 1.99, utilisé par la CI) ([4f05676](https://github.com/yazouv/BoringWindows/commit/4f056761f06ab490a370f529a8e19762d8872c1e))


### Performances

* **claude:** hooks async sauf la demande de permission ([6107080](https://github.com/yazouv/BoringWindows/commit/61070808058a5bbcfd0f26dbacdf9ac2990256ce))


### Documentation

* documentation bilingue (français / anglais) ([7c53d86](https://github.com/yazouv/BoringWindows/commit/7c53d862be84f9f0ceeab85004ec0cd4728aa70f))
* images optimisées (ImgBot) aussi pour la version française ([3352b1b](https://github.com/yazouv/BoringWindows/commit/3352b1bd59873dceb7b6fb57de458c864e56b096))
* plan du projet BoringWindows ([73e80cc](https://github.com/yazouv/BoringWindows/commit/73e80ccfe2f30e0652e6b6d9bed2ac7ad1fe8b3d))
* **plan:** phase 3.5 — fenêtre de réglages et documentation en ligne ([cd8e479](https://github.com/yazouv/BoringWindows/commit/cd8e4795514cdce574eeeba003f9fb80e631a1cc))
* site de documentation (mdBook, GitHub Pages) ([2ee2cb6](https://github.com/yazouv/BoringWindows/commit/2ee2cb604d58480967e8a9717d73917df3fd34f7))
