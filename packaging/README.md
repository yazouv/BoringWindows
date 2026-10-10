# Packaging

## Installeur (Inno Setup)

`installer/boringwindows.iss` est compilé par le job Windows (étape « Installeur ») de
`.github/workflows/build.yml` à chaque release et attaché sous
`boringwindows-windows-x64-setup.exe` (+ `.sha256`). En local :

```powershell
iscc /DAppVersion=0.1.0 /DSourceExe=C:\chemin\boringwindows.exe installer\boringwindows.iss
```

Installation par utilisateur, sans droits admin. La config
(`%APPDATA%\BoringWindows`) est conservée à la désinstallation.

## winget

`packaging/winget/` contient le modèle de manifeste (identifiant
`Yazouv.BoringWindows`).

1. **Première soumission, à la main** une fois une release avec installeur
   publiée : remplacer la version, l'URL et l'empreinte dans
   `Yazouv.BoringWindows.installer.yaml`, vérifier avec
   `winget validate packaging\winget`, puis proposer les trois fichiers dans
   un PR à `microsoft/winget-pkgs` (dans
   `manifests/y/Yazouv/BoringWindows/<version>/`), ou utiliser
   `wingetcreate new <url de l'installeur>`.
2. **Ensuite, automatique** : ajouter un secret `WINGET_TOKEN` (jeton GitHub
   classique, portée `public_repo`) au dépôt ; l'étape `winget` du job Windows de la release
   ouvre alors un PR de mise à jour avec `wingetcreate update`. Sans le secret,
   il ne fait rien.
