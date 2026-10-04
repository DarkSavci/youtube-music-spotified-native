; Installer for Youtube Music Spotified (native), built with Inno Setup 6.
;
;   scripts\package.ps1
;   iscc packaging\windows\spotified.iss
;
; Installs for the current user, with no administrator prompt, from the
; folder package.ps1 gathers. The AppId is this app's own: it installs
; beside the Electron app, not over it.

#define Name "Youtube Music Spotified"
; What the system calls it: shortcuts, the Start menu, the list of installed
; apps. Told apart from the Electron app, whose shortcuts carry the bare name
; and would otherwise be overwritten.
#define Title "Youtube Music Spotified Native"
#define Exe "Youtube Music Spotified.exe"
#ifndef Version
  #define Version "0.1.0"
#endif

[Setup]
AppId={{6B0D2B0B-6C6E-4B8F-9B55-6E1D5C0C4E11}
AppName={#Title}
AppVersion={#Version}
AppPublisher=DarkSavci
DefaultDirName={localappdata}\Programs\{#Title}
DefaultGroupName={#Title}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir=..\..\dist
OutputBaseFilename=youtube-music-spotified-native-{#Version}-setup
SetupIconFile=..\..\crates\app\assets\icon.ico
UninstallDisplayIcon={app}\{#Exe}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
; An update replaces a running copy: ask it to close, and do not start it
; again behind the person's back.
CloseApplications=yes
RestartApplications=no

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; Flags: unchecked

[Files]
Source: "..\..\dist\{#Name}\*"; DestDir: "{app}"; Flags: recursesubdirs ignoreversion

[Icons]
Name: "{group}\{#Title}"; Filename: "{app}\{#Exe}"
Name: "{userdesktop}\{#Title}"; Filename: "{app}\{#Exe}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#Exe}"; Description: "{cm:LaunchProgram,{#Title}}"; Flags: nowait postinstall skipifsilent

; An update run by the app itself asks to be started again when done.
Filename: "{app}\{#Exe}"; Flags: nowait; Check: Relaunch

[Code]
function Relaunch: Boolean;
begin
  Result := ExpandConstant('{param:RELAUNCH|0}') = '1';
end;

[Registry]
; The start-with-Windows entry the app may have written goes with it.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueName: "YoutubeMusicSpotifiedNative"; Flags: dontcreatekey uninsdeletevalue
