; SoftEcho — установщик Windows (Inno Setup 6).
; Собирается скриптом scripts/package-windows-installer.sh из portable-папки.
;
; Параметры (/D):
;   MyAppVersion  — версия (по умолчанию 0.2.0)
;   SourceDir     — абсолютный путь к dist/softecho-windows-x86_64-{text|asr}
;   NameSuffix    — text | asr (имя выходного файла)
;   OutputDir     — каталог для Setup.exe (по умолчанию рядом с репо /dist)

#ifndef MyAppVersion
  #define MyAppVersion "0.0.0"
#endif
#ifndef NameSuffix
  #define NameSuffix "asr"
#endif
#ifndef SourceDir
  #define SourceDir "..\..\dist\softecho-windows-x86_64-asr"
#endif
#ifndef OutputDir
  #define OutputDir "..\..\dist"
#endif

#define MyAppName "SoftEcho"
#define MyAppPublisher "SoftEcho"
#define MyAppURL "https://github.com/ArtVal/softecho"
#define MyAppExeName "softecho.exe"

[Setup]
; Стабильный Id — обновления не плодят вторую копию в «Программы и компоненты».
AppId={{E8F3A2C1-9B4D-4E7A-8F2C-1A0B3D5E7F90}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}/releases
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
AllowNoIcons=yes
InfoBeforeFile=install-info-ru.txt
OutputDir={#OutputDir}
OutputBaseFilename=softecho-windows-x86_64-setup-{#NameSuffix}
SetupIconFile=..\..\assets\softecho.ico
UninstallDisplayIcon={app}\{#MyAppExeName}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
; По умолчанию без админа (папка пользователя). Можно выбрать «для всех» в диалоге.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
DisableProgramGroupPage=yes
CloseApplications=yes

[Languages]
Name: "russian"; MessagesFile: "compiler:Languages\Russian.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#SourceDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#MyAppName}}"; Flags: nowait postinstall skipifsilent
