@echo off
setlocal enabledelayedexpansion

set "GREEN=[92m"
set "YELLOW=[93m"
set "RED=[91m"
set "NC=[0m"

echo ╔══════════════════════════════════════════╗
echo ║     Auto-FG Setup                        ║
echo ╚══════════════════════════════════════════╝
echo.

:: ---- 1. Install Rust if missing ----
where cargo >nul 2>nul
if %errorlevel% neq 0 (
    echo %YELLOW%[WARN]%NC% Rust not found. Installing...
    curl -sSfLo "%TEMP%\rustup-init.exe" https://win.rustup.rs/x86_64
    "%TEMP%\rustup-init.exe" -y
    call "%USERPROFILE%\.cargo\env"
)
for /f "tokens=*" %%i in ('cargo --version') do echo [INFO] Rust: %%i

:: ---- 2. Build ----
echo [INFO] Building (this may take a while)...
cargo build --release

:: ---- 3. Start Menu shortcut ----
set "STARTMENU=%APPDATA%\Microsoft\Windows\Start Menu\Programs\Auto-FG"
if not exist "%STARTMENU%" mkdir "%STARTMENU%"

:: Create a VBS script to create shortcuts (more reliable)
set "VBS=%TEMP%\shortcut.vbs"
echo Set WshShell = WScript.CreateObject("WScript.Shell") > "%VBS%"
echo Set Shortcut = WshShell.CreateShortcut("%STARTMENU%\Auto-FG.lnk") >> "%VBS%"
echo Shortcut.TargetPath = "%~dp0target\release\Ffast-auto-downloader.exe" >> "%VBS%"
echo Shortcut.WorkingDirectory = "%~dp0" >> "%VBS%"
echo Shortcut.Description = "Auto-FG - FitGirl repack downloader" >> "%VBS%"
echo Shortcut.Save >> "%VBS%"

echo Set Shortcut = WshShell.CreateShortcut("%STARTMENU%\Auto-FG (get links).lnk") >> "%VBS%"
echo Shortcut.TargetPath = "%~dp0target\release\get-links.exe" >> "%VBS%"
echo Shortcut.WorkingDirectory = "%~dp0" >> "%VBS%"
echo Shortcut.Save >> "%VBS%"

echo Set Shortcut = WshShell.CreateShortcut("%STARTMENU%\Auto-FG (download).lnk") >> "%VBS%"
echo Shortcut.TargetPath = "%~dp0target\release\download.exe" >> "%VBS%"
echo Shortcut.WorkingDirectory = "%~dp0" >> "%VBS%"
echo Shortcut.Save >> "%VBS%"

:: Desktop shortcut
echo Set Shortcut = WshShell.CreateShortcut("%USERPROFILE%\Desktop\Auto-FG.lnk") >> "%VBS%"
echo Shortcut.TargetPath = "%~dp0target\release\Ffast-auto-downloader.exe" >> "%VBS%"
echo Shortcut.WorkingDirectory = "%~dp0" >> "%VBS%"
echo Shortcut.Description = "Auto-FG - FitGirl repack downloader" >> "%VBS%"
echo Shortcut.Save >> "%VBS%"

cscript //nologo "%VBS%"
del "%VBS%"
echo [INFO] Shortcuts created in Start Menu and Desktop

:: ---- 4. Done ----
echo.
echo ╔══════════════════════════════════════════╗
echo ║           All Set!                       ║
echo ╚══════════════════════════════════════════╝
echo.
echo   Run it from Start Menu or Desktop shortcut
echo   Re-run this script to update
echo.
pause
