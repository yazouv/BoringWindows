; Installeur Inno Setup 6 de BoringWindows (installation par utilisateur, sans droits admin).
;
; Compilation (depuis la racine du dépôt) :
;   iscc /DAppVersion=0.1.0 /DSourceExe=chemin\boringwindows-windows-x64.exe installer\boringwindows.iss
; Résultat : installer\Output\boringwindows-windows-x64-setup.exe

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef SourceExe
  #define SourceExe "..\target\release\boringwindows.exe"
#endif

[Setup]
AppId={{B6A8F0E2-5C1D-4E7A-9B3F-2D4C8E1A7F60}
AppName=BoringWindows
AppVersion={#AppVersion}
AppVerName=BoringWindows {#AppVersion}
AppPublisher=Yazouv
AppPublisherURL=https://github.com/yazouv/BoringWindows
AppSupportURL=https://github.com/yazouv/BoringWindows/issues
AppUpdatesURL=https://github.com/yazouv/BoringWindows/releases
DefaultDirName={autopf}\BoringWindows
DefaultGroupName=BoringWindows
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
OutputDir=Output
OutputBaseFilename=boringwindows-windows-x64-setup
Compression=lzma2
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
WizardStyle=modern
UninstallDisplayName=BoringWindows
UninstallDisplayIcon={app}\boringwindows.exe
; L'app tient un mutex d'instance unique : Inno propose de la fermer avant de remplacer le fichier.
AppMutex=BoringWindows.SingleInstance
CloseApplications=yes
; L'app se met à jour toute seule en remplaçant son exécutable : le dossier doit rester modifiable.
DisableDirPage=auto

[Languages]
Name: "french"; MessagesFile: "compiler:Languages\French.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; Flags: unchecked

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; DestName: "boringwindows.exe"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\BoringWindows"; Filename: "{app}\boringwindows.exe"
Name: "{autoprograms}\BoringWindows (Réglages)"; Filename: "{app}\boringwindows.exe"; Parameters: "--settings"
Name: "{autodesktop}\BoringWindows"; Filename: "{app}\boringwindows.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\boringwindows.exe"; Description: "{cm:LaunchProgram,BoringWindows}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
; Fermer l'île avant de supprimer l'exécutable ; la config (%APPDATA%\BoringWindows) est conservée.
Filename: "{sys}\taskkill.exe"; Parameters: "/F /IM boringwindows.exe"; Flags: runhidden; RunOnceId: "StopBoringWindows"
