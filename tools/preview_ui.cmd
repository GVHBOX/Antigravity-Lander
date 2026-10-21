@echo off
setlocal

rem Launch the Slint live preview for the cockpit UI.
rem Editing any .slint file refreshes the window within seconds,
rem without triggering a Rust rebuild.

set "VIEWER=C:\Users\GVH\.cargo\bin\slint-viewer.exe"
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
