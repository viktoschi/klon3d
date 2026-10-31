@echo off
rem ===========================================================================
rem  klon3d — Fenster fuer die Teileumrisse eines Seamly2D-Musters
rem ===========================================================================
setlocal
set EXE=%~dp0target\release\klon3d-fenster.exe

if not exist "%EXE%" (
  echo   klon3d-fenster wurde nicht gebaut.
  echo   In D:\ai\rea\klon3d:  cargo build --release
  echo.
  pause
  exit /b 1
)

set M=D:\ai\rea\klon3d\crates\klon3d-pattern\tests\muster
"%EXE%" "%M%\bluse2_v26.sm2d" "%M%\bluse3_v1.sm2d" "%M%\bustier_v1.sm2d"
if errorlevel 1 pause
endlocal