@echo off
rem Final confirming gate run after S14 re-pin (161.36) + S7 re-attestation (multi_voice).
setlocal
call "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvarsall.bat" amd64 >nul
cd /d C:\Users\CarlosRuizMartínez\workspace\Meetily-flash
set CMAKE_GENERATOR=Ninja
set CL=/FS
set CARGO_TARGET_DIR=C:\mf
set MEETIFY_LIVE_DIAG=1
cargo test --release --features vulkan --test ear_truth_gate -- --ignored --nocapture
endlocal
