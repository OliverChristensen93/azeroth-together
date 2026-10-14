@echo off
rem Azeroth Together: double-click to play (build it first: cargo build --release -p azeroth-together).
cd /d "%~dp0"
if not exist "target\release\azeroth-together.exe" (
    echo The game is not built yet. Run: cargo build --release -p azeroth-together
    pause
    exit /b 1
)
"target\release\azeroth-together.exe"
if errorlevel 1 pause
