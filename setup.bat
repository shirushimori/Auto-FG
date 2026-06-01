@echo off
setlocal enabledelayedexpansion

set "BOLD=[1m"
set "GREEN=[92m"
set "YELLOW=[93m"
set "RED=[91m"
set "NC=[0m"

echo %BOLD%╔══════════════════════════════════════════╗%NC%
echo %BOLD%║  FuckingFast FitGirl Download Automator  ║%NC%
echo %BOLD%║         Automated Setup Script           ║%NC%
echo %BOLD%╚══════════════════════════════════════════╝%NC%
echo.

:: ---- 1. Install Rust (via rustup) if missing ----
where cargo >nul 2>nul
if %errorlevel% neq 0 (
    echo %YELLOW%[WARN]%NC% Rust is not installed.
    echo   Downloading rustup-init.exe...
    curl -sSfLo "%TEMP%\rustup-init.exe" https://win.rustup.rs/x86_64
    if !errorlevel! neq 0 (
        echo %RED%[ERROR]%NC% Failed to download rustup-init.exe
        echo   Download manually from: https://rustup.rs
        pause
        exit /b 1
    )
    echo   Installing Rust (this may take a few minutes)...
    "%TEMP%\rustup-init.exe" -y
    if !errorlevel! neq 0 (
        echo %RED%[ERROR]%NC% Rust installation failed
        pause
        exit /b 1
    )
    :: Refresh PATH to include cargo
    call "%USERPROFILE%\.cargo\env"
)

for /f "tokens=*" %%i in ('cargo --version') do echo %GREEN%[INFO]%NC% Rust found: %%i

:: ---- 2. Ensure Visual Studio Build Tools (for Rust MSVC) ----
echo %GREEN%[INFO]%NC% Checking Visual Studio Build Tools...
:: Rust MSVC toolchain needs VC++ build tools; if not found, suggest installing
rustc -v | findstr "msvc" >nul
if %errorlevel% equ 0 (
    :: MSVC toolchain - check for link.exe
    where link >nul 2>nul
    if !errorlevel! neq 0 (
        echo %YELLOW%[WARN]%NC% MSVC toolchain selected but link.exe not found.
        echo   Install Visual Studio Build Tools from:
        echo   https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022
        echo   Or switch to GNU toolchain: rustup default stable-gnu
        echo.
        echo %YELLOW%[INFO]%NC% Attempting to continue anyway (build may fail)...
    ) else (
        echo %GREEN%[INFO]%NC% Visual Studio Build Tools found.
    )
) else (
    echo %GREEN%[INFO]%NC% GNU toolchain in use - no extra tools needed.
)

:: ---- 3. Git setup (if inside a git repo) ----
git rev-parse --git-dir >nul 2>nul
if %errorlevel% equ 0 (
    echo %GREEN%[INFO]%NC% Git repository detected.
    git submodule update --init --recursive 2>nul || rem
    echo %GREEN%[INFO]%NC% Git repository configured.
) else (
    echo %YELLOW%[WARN]%NC% Not a git repository. Skipping git setup.
)

:: ---- 4. Build ----
echo.
echo %GREEN%[INFO]%NC% Building project (release mode^)...
cargo build --release

if %errorlevel% neq 0 (
    echo %RED%[ERROR]%NC% Build failed. Check the output above for errors.
    pause
    exit /b 1
)

:: ---- 5. Install binaries ----
set "INSTALL_DIR=%USERPROFILE%\.cargo\bin"

echo.
echo %GREEN%[INFO]%NC% Binaries built. They are available in:
echo   target\release\Ffast-auto-downloader.exe
echo   target\release\get-links.exe
echo   target\release\download.exe
echo.
echo %GREEN%[INFO]%NC% You can run them directly with:
echo   cargo run --release
echo   cargo run --release --bin get-links
echo   cargo run --release --bin download

:: ---- 6. Done ----
echo.
echo %BOLD%╔══════════════════════════════════════════╗%NC%
echo %BOLD%║           Setup Complete!                ║%NC%
echo %BOLD%╚══════════════════════════════════════════╝%NC%
echo.
echo   Installed commands:
echo     cargo run --release     — Launch the GUI app
echo     fitgirl-downloader     — (via PATH if .cargo\bin is added)
echo.
echo   Run the GUI:
echo     cargo run --release
echo.

pause
