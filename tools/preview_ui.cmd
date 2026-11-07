@echo off
setlocal

rem Launch the Slint live preview for the cockpit UI.
rem Editing any .slint file refreshes the window within seconds,
rem without triggering a Rust rebuild.

set "VIEWER=%USERPROFILE%\.cargo\bin\slint-viewer.exe"
for /f "delims=" %%i in ('where slint-viewer 2^>nul') do set "VIEWER=%%i"
set "ROOT=%~dp0.."

if not exist "%VIEWER%" (
    echo slint-viewer not found: %VIEWER%
    echo Install it with: cargo install slint-viewer
    pause
    exit /b 1
)

pushd "%ROOT%"
"%VIEWER%" --auto-reload src/cockpit.slint -I assets
popd

endlocal
