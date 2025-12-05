@echo off
setlocal enabledelayedexpansion
REM Simple backup script for a project directory

set SRC=%~1
set DEST=%~2

if "%SRC%"=="" (
  echo Usage: backup.bat SOURCE DEST
  exit /b 1
)

if "%DEST%"=="" (
  echo Usage: backup.bat SOURCE DEST
  exit /b 1
)

if not exist "%SRC%" (
  echo Source path not found: %SRC%
  exit /b 1
)

if not exist "%DEST%" (
  echo Creating destination %DEST%
  mkdir "%DEST%"
)

set TIMESTAMP=%DATE:~10,4%-%DATE:~4,2%-%DATE:~7,2%_%TIME:~0,2%-%TIME:~3,2%
set LOG=%DEST%\backup_!TIMESTAMP!.log

echo Starting backup at %DATE% %TIME% >"%LOG%"
echo Source: %SRC% >>"%LOG%"
echo Dest: %DEST% >>"%LOG%"

echo Copying files...
xcopy /E /I /Y "%SRC%" "%DEST%\current" >>"%LOG%" 2>&1
if errorlevel 1 (
  echo Copy failed. See log: %LOG%
  exit /b 1
)

echo Archiving previous backup if exists...
if exist "%DEST%\previous" (
  rmdir /S /Q "%DEST%\previous" >>"%LOG%" 2>&1
)
if exist "%DEST%\current" (
  ren "%DEST%\current" previous >>"%LOG%" 2>&1
)

if exist "%DEST%\previous" (
  ren "%DEST%\previous" backup_!TIMESTAMP! >>"%LOG%" 2>&1
)

echo Backup completed successfully. Log saved to %LOG%
endlocal
