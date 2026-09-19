@echo off
rem Construit RunTerm.exe et l'installateur NSIS avec les outils Windows.
rem Utilise node\ et toolchain\ portables s'ils existent a la racine du dossier.
setlocal
cd /d "%~dp0.."

set "VSWHERE=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe"
if not exist "%VSWHERE%" (
  echo Visual Studio Build Tools introuvables.
  exit /b 1
)
for /f "usebackq delims=" %%i in (`"%VSWHERE%" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set "VSDIR=%%i"
if not defined VSDIR (
  echo Outils C++ de Visual Studio introuvables.
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
call npm.cmd run tauri -- build
exit /b %ERRORLEVEL%
