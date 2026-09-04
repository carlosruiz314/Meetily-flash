@echo off
rem Ear-truth fixture gate runner (change hybrid-diarization-engine, task 1.4).
rem Compiles the test crate, runs the env-gated gate on the real meeting audio,
rem and records the FULL output (compile + results) into gate-runs/<ts>.log so
rem acceptance evidence is inspectable without re-running.
setlocal
call "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvarsall.bat" amd64 >nul
cd /d C:\Users\CarlosRuizMartínez\workspace\Meetily-flash
set CMAKE_GENERATOR=Ninja
set CL=/FS
set CARGO_TARGET_DIR=C:\mf
set MEETIFY_LIVE_DIAG=1

set TS=%DATE:~-4%%DATE:~4,2%%DATE:~7,2%-%TIME:~0,2%%TIME:~3,2%%TIME:~6,2%
set TS=%TS: =0%
set OUT=%~dp0gate-runs\%TS%.log
if not exist "%~dp0gate-runs" mkdir "%~dp0gate-runs"

echo gate run %TS% > "%OUT%"
cargo test --release --features vulkan --test ear_truth_gate -- --ignored --nocapture >> "%OUT%" 2>&1
type "%OUT%"
echo.
echo recorded: %OUT%
endlocal
