@echo off
rem Fresh-input gate: recompute production frame masses (no cache) + piece dump.
setlocal
call "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvarsall.bat" amd64 >nul
cd /d C:\Users\CarlosRuizMartínez\workspace\Meetily-flash
set CMAKE_GENERATOR=Ninja
set CL=/FS
set CARGO_TARGET_DIR=C:\mf
set MEETIFY_LIVE_DIAG=1
set MEETIFY_ENGINE_DEBUG=1
cargo test --release --features vulkan --test ear_truth_gate -- --ignored --nocapture
endlocal
