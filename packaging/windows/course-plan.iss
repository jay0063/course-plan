; Inno Setup script for 小学课程管理计划 (CoursePlan)
;
; Build on Windows with:
;   1. cargo build --release --locked
;   2. ISCC.exe packaging\windows\course-plan.iss
;
; Produces dist\CoursePlan-Setup-<version>.exe and, when ZIP is requested,
; dist\CoursePlan-<version>-windows-x64.zip.

#define AppName "课程计划"
#define AppNameEn "CoursePlan"
#define AppVersion "0.1.0"
#define AppExe "course-plan.exe"
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
OutputDir=..\dist
OutputBaseFilename={#AppNameEn}-Setup-{#AppVersion}
SetupIconFile=..\packaging\icons\course-plan.ico
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
Source: "..\target\release\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion isreadme skipifsourcedoesntexist

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{group}\{cm:UninstallProgram,{#AppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppNameEn}}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; Remove per-user data written by the app. Shared documents are left alone.
Type: filesandordirs; Name: "{localappdata}\courseplan"

[Code]
function InitializeSetup(): Boolean;
begin
  Result := True;
end;
