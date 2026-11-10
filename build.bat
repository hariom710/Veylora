@echo off
setlocal

rem Locate the MSVC toolchain via vswhere instead of a hardcoded path, so this
rem survives Visual Studio updates and fails loudly when the toolchain is gone.
set "VSWHERE=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe"
if not exist "%VSWHERE%" goto :no_toolchain

set "VSINSTALL="
for /f "usebackq delims=" %%i in (`"%VSWHERE%" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set "VSINSTALL=%%i"
if not defined VSINSTALL goto :no_toolchain

if not exist "%VSINSTALL%\VC\Auxiliary\Build\vcvars64.bat" goto :no_toolchain
call "%VSINSTALL%\VC\Auxiliary\Build\vcvars64.bat" >nul 2>&1
if errorlevel 1 goto :no_toolchain

cd /d D:\projets\Veylora\src-tauri
if errorlevel 1 exit /b 1
cargo %*
exit /b %errorlevel%

:no_toolchain
echo.
echo ERROR: No MSVC C++ toolchain found.
echo Install "Build Tools for Visual Studio" with the "Desktop development with C++"
echo workload, then re-run this script.
echo.
exit /b 1
