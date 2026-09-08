#ifndef AppVersion
  #define AppVersion "0.2.0"
#endif
#ifndef PackageDir
  #error PackageDir must point to the prepared portable directory
#endif

[Setup]
AppId={{851CA4BC-D5A1-4782-98C4-A24D89742AD2}
AppName=WinShell
AppVersion={#AppVersion}
AppPublisher=WinShell contributors
AppPublisherURL=https://github.com/neko233-com/winshell
AppSupportURL=https://github.com/neko233-com/winshell/issues
AppUpdatesURL=https://github.com/neko233-com/winshell/releases
DefaultDirName={localappdata}\Programs\WinShell
DefaultGroupName=WinShell
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763
DisableProgramGroupPage=yes
UninstallDisplayIcon={app}\winshell.exe
SetupIconFile=..\assets\winshell.ico
OutputDir=..\dist
OutputBaseFilename=winshell-{#AppVersion}-windows-x64-setup
Compression=lzma2/fast
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
CloseApplicationsFilter=winshell.exe
RestartApplications=no
LicenseFile=..\LICENSE
LanguageDetectionMethod=uilanguage
ShowLanguageDialog=auto
SetupLogging=yes

[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "zh_CN"; MessagesFile: "languages\ChineseSimplified.isl"
Name: "zh_TW"; MessagesFile: "languages\ChineseTraditional.isl"
Name: "ja"; MessagesFile: "compiler:Languages\Japanese.isl"
Name: "ko"; MessagesFile: "compiler:Languages\Korean.isl"
Name: "de"; MessagesFile: "compiler:Languages\German.isl"
Name: "fr"; MessagesFile: "compiler:Languages\French.isl"
Name: "es"; MessagesFile: "compiler:Languages\Spanish.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; Flags: unchecked

[Files]
Source: "{#PackageDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\WinShell"; Filename: "{app}\winshell.exe"; WorkingDir: "{userprofile}"
Name: "{autodesktop}\WinShell"; Filename: "{app}\winshell.exe"; WorkingDir: "{userprofile}"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\App Paths\winshell.exe"; ValueType: string; ValueData: "{app}\winshell.exe"; Flags: uninsdeletekey

[Run]
Filename: "{app}\winshell.exe"; Description: "{cm:LaunchProgram,WinShell}"; Flags: nowait postinstall skipifsilent
