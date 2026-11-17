@echo off
rem Full-chain gate runner (change gap-speech-voice-attribution, task 4.2).
rem lib tests + synthetic gate + env-gated ear gate on the real meeting audio;
rem full output recorded into openspec/exploration/ (task 4.2 evidence rule).
setlocal
set ROOT=%~dp0..\..\..
cd /d "%ROOT%"
set CARGO_TARGET_DIR=C:\mf
set MEETIFY_LIVE_DIAG=1
set MEETIFY_RENDER_PRINT=1

set TS=%DATE:~-4%%DATE:~4,2%%DATE:~7,2%-%TIME:~0,2%%TIME:~3,2%%TIME:~6,2%
set TS=%TS: =0%
set OUT=%ROOT%\openspec\exploration\gap-speech-4.2-chain-%TS%.log

echo chain run %TS% > "%OUT%"
echo === cargo test --lib === >> "%OUT%"
cargo test --release --features vulkan --lib >> "%OUT%" 2>&1
echo === synthetic gate === >> "%OUT%"
cargo test --release --features vulkan --test engine_synthetic_gate >> "%OUT%" 2>&1
echo === ear gate (live) === >> "%OUT%"
cargo test --release --features vulkan --test ear_truth_gate -- --ignored --nocapture >> "%OUT%" 2>&1
findstr /C:"test result" /C:"GATE SUMMARY" /C:"RENDER:" "%OUT%"
echo.
echo recorded: %OUT%
endlocal
