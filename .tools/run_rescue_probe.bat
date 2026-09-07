@echo off
rem Gap-rescue de-risk probe (change gap-speech-voice-attribution, tasks 1.1-1.6).
setlocal
call "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvarsall.bat" amd64 >nul
cd /d C:/Users/CarlosRuizMartínez/workspace/Meetily-flash
set CMAKE_GENERATOR=Ninja
set CL=/FS
set CARGO_TARGET_DIR=C:/mf
set MEETIFY_LIVE_DIAG=1
cargo test --release --features vulkan --test gap_rescue_v3_probe -- --ignored --nocapture
endlocal
