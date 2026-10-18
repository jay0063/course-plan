; Inno Setup script for 小学课程管理计划 (CoursePlan)
;
; Build on Windows with the helper script (preferred -- it reads the version
; from Cargo.toml and resolves the built executable for you):
;   pwsh -File packaging\windows\build-windows.ps1
;
; To run ISCC directly, pass the version and the built executable explicitly so
; they cannot drift from Cargo.toml / the active target directory. The defaults
; below are only fallbacks for a bare `ISCC.exe packaging\windows\course-plan.iss`:
;   cargo build --release --locked --target x86_64-pc-windows-msvc
;   ISCC.exe /DAppVersion=0.1.0 ^
;            /DExePath=target\x86_64-pc-windows-msvc\release\course-plan.exe ^
;            packaging\windows\course-plan.iss
;
; All relative paths below are repository-root-relative thanks to
; SourceDir=..\.. -- this script lives in packaging\windows\, and Inno resolves
; [Setup] file directives, [Files] Source, and OutputDir against SourceDir.
;
; Produces dist\CoursePlan-Setup-<version>.exe.

#define AppName "课程计划"
#define AppNameEn "CoursePlan"
#define AppExe "course-plan.exe"
#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif
#ifndef ExePath
  #define ExePath "target\release\course-plan.exe"
#endif
#define AppPublisher "CoursePlan"
#define AppId "{{6F3A1C42-9B7E-4C51-9C2B-1A6E5D0F7B34}"

[Setup]
AppId={#AppId}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL=https://github.com/your-org/course-plan
AppSupportURL=https://github.com/your-org/course-plan/issues
DefaultDirName={autopf}\{#AppNameEn}
DefaultGroupName={#AppNameEn}
DisableProgramGroupPage=yes
LicenseFile=
; Resolve every relative path against the repository root, not this folder.
SourceDir=..\..
OutputDir=dist
OutputBaseFilename={#AppNameEn}-Setup-{#AppVersion}
SetupIconFile=packaging\icons\course-plan.ico
UninstallDisplayIcon={app}\{#AppExe}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
; x64compatible covers Windows 10 and 11 on both x64 and Arm64.
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequiredOverridesAllowed=dialog

[Languages]
Name: "chinese"; MessagesFile: "compiler:Default.isl"
Name: "english"; MessagesFile: "compiler:English.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
; The application binary. Sign it before this step for a public release.
Source: "{#ExePath}"; DestDir: "{app}"; Flags: ignoreversion
Source: "README.md"; DestDir: "{app}"; Flags: ignoreversion isreadme skipifsourcedoesntexist

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{group}\{cm:UninstallProgram,{#AppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppNameEn}}"; Flags: nowait postinstall skipifsilent

; NOTE: user data is intentionally NOT deleted on uninstall. The app stores it
; under %APPDATA%\courseplan\CoursePlan\data\course-plan\ (Roaming), which is
; separate from {app}; uninstalling keeps it so a reinstall/upgrade loses
; nothing. See packaging/CROSS-COMPILE.md for the manual removal path.

[Code]
function InitializeSetup(): Boolean;
begin
  Result := True;
end;
