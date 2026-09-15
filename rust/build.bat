@echo off
cd /d "%~dp0"

rem Static CRT build: no external runtime dependency
set RUSTFLAGS=-C target-feature=+crt-static
cargo build --release --target x86_64-pc-windows-msvc
if errorlevel 1 goto :fail

if not exist dist mkdir dist
copy /y "target†_64-pc-windows-msvcelease\screen_stripe.exe" "dist\screen_stripe.exe" >nul
for %%A in ("dist\screen_stripe.exe") do echo Built: %%~fA  (%%~zA bytes)
goto :eof

:fail
echo Build FAILED
exit /b 1
