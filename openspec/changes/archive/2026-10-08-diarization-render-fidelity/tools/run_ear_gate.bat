@echo off
REM diarization-render-fidelity task 5.5: named runner recording the
REM env-gated ear-truth gate's output at every verification point. The
REM output file lives in gate-runs/ (gitignored + pre-push-protected) and
REM carries meeting-derived text on violations - it NEVER enters the repo.
REM Token-only census lines are safe to paste anywhere; CENSUS-TEXT lines
REM (verbatim stream text, MEETIFY_RENDER_PRINT) are terminal-only.
REM ASCII-only + CRLF: cmd's parser executes fragments of comment lines
REM when a .bat has LF-only endings or non-ASCII text.
setlocal
set TS=%DATE:~-4%%DATE:~4,2%%DATE:~7,2%-%TIME:~0,2%%TIME:~3,2%%TIME:~6,2%
set TS=%TS: =0%
set OUT=%~dp0..\gate-runs\%TS%.log
if not exist "%~dp0..\gate-runs" mkdir "%~dp0..\gate-runs"
if "%MEETIFY_LIVE_DIAG%"=="" (
  echo MEETIFY_LIVE_DIAG is not set - refusing to run a no-op gate 1>&2
  exit /b 1
)
if "%MEETIFY_GATE_LANG%"=="" (
  echo MEETIFY_GATE_LANG is not set - stream decode would degrade (no synthesis) 1>&2
  exit /b 1
)
if "%MEETIFY_GATE_WHISPER_MODEL%"=="" (
  echo MEETIFY_GATE_WHISPER_MODEL is not set - the S16 replay needs a real model 1>&2
  exit /b 1
)
echo Recording to %OUT%
cargo test -j 2 --test ear_truth_gate -- --ignored --nocapture >> "%OUT%" 2>&1
echo Exit code %ERRORLEVEL% - recorded output at %OUT%
exit /b %ERRORLEVEL%
