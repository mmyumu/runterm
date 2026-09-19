@echo off
rem Builds RunTerm.exe and the NSIS installer with the Windows tools.
rem Uses portable node\ and toolchain\ if they exist at the root of the folder.
setlocal
cd /d "%~dp0.."

set "VSWHERE=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe"
if not exist "%VSWHERE%" (
  echo Visual Studio Build Tools not found.
  exit /b 1
)
for /f "usebackq delims=" %%i in (`"%VSWHERE%" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set "VSDIR=%%i"
if not defined VSDIR (
  echo Visual Studio C++ tools not found.
  exit /b 1
)
call "%VSDIR%\VC\Auxiliary\Build\vcvars64.bat" >nul
if errorlevel 1 exit /b 1

if exist "%CD%\node\node.exe" set "PATH=%CD%\node;%PATH%"
if exist "%CD%\toolchain\bin\rustc.exe" (
  set "PATH=%CD%\toolchain\bin;%PATH%"
  set "CARGO_HOME=%CD%\.cargo"
  set "RUSTC=%CD%\toolchain\bin\rustc.exe"
  set "RUSTDOC=%CD%\toolchain\bin\rustdoc.exe"
)
set "npm_config_cache=%CD%\.npm-cache"

call npm.cmd ci
if errorlevel 1 exit /b 1
rem Without the updater key the installer is built unsigned and cannot be served as an update.
if defined TAURI_SIGNING_PRIVATE_KEY (
  call npm.cmd run tauri -- build
) else (
  call npm.cmd run tauri -- build --no-sign
)
exit /b %ERRORLEVEL%
