@echo off
rem Closure-gap probe runner (exploration: what closes the S4/S5/S6 gap).
setlocal
call "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvarsall.bat" amd64 >nul
cd /d C:\Users\CarlosRuizMartínez\workspace\Meetily-flash
set CMAKE_GENERATOR=Ninja
set CL=/FS
set CARGO_TARGET_DIR=C:\mf
set MEETIFY_LIVE_DIAG=1
cargo test --release --features vulkan --test closure_gap_probe -- --ignored --nocapture
endlocal
