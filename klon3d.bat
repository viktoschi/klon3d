@echo off
rem ============================================================================
rem  klon3d - Kommandozeile fuer Seamly2D-Muster
rem
rem  Aufruf
rem    klon3d.bat                       Hilfe anzeigen
rem    klon3d.bat info   <muster.sm2d>   Muster lesen, Aufbau melden
rem    klon3d.bat punkte <muster.sm2d>   Konstruktion ausrechnen
rem    klon3d.bat bauen                  neu kompilieren
rem
rem  Ohne Argument oder mit "info"/"punkte" ohne Datei wird ein Muster
rem  VORGESCHLAGEN, damit ein Doppelklick etwas zeigt.
rem ============================================================================
setlocal
set "ROOT=%~dp0"
set "EXE=%ROOT%target\release\klon3d.exe"
set "CARGO_HOME=%ROOT%..\tools\rust\cargo"
set "RUSTUP_HOME=%ROOT%..\tools\rust\rustup"
set "PATH=%CARGO_HOME%\bin;%PATH%"
chcp 65001 >nul

if /I "%~1"=="bauen" goto bauen

if not exist "%EXE%" goto noch_nicht_gebaut

if "%~1"=="" goto zeigen
if /I "%~1"=="info" if "%~2"=="" goto zeigen
if /I "%~1"=="punkte" if "%~2"=="" goto zeigen

"%EXE%" %*
goto ende

:bauen
echo Kompiliere klon3d (release) ...
echo.
pushd "%ROOT%"
cargo build --release
set RC=%ERRORLEVEL%
popd
echo.
if not "%RC%"=="0" echo BUILD FEHLGESCHLAGEN
if "%RC%"=="0" echo Fertig: %EXE%
goto ende_mit_pause

:noch_nicht_gebaut
echo.
echo   klon3d.exe ist noch nicht gebaut.
echo.
echo   Bauen mit:   klon3d.bat bauen
echo.
goto ende_mit_pause

:zeigen
rem Ohne Datei: ein Muster aus dem Arbeitsumfeld vorschlagen.
set "VORSCHLAG=%ROOT%crates\klon3d-pattern\tests\muster\bluse2_v26.sm2d"
if not exist "%VORSCHLAG%" goto nur_hilfe
echo.
echo   Kein Muster angegeben - zeige ein Beispiel:
echo   %VORSCHLAG%
echo.
"%EXE%" info "%VORSCHLAG%"
goto ende_mit_pause

:nur_hilfe
"%EXE%"
goto ende_mit_pause

:ende_mit_pause
echo.
pause
goto :eof

:ende
endlocal & exit /b %ERRORLEVEL%