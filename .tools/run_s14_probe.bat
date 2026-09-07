@echo off
rem S14 boundary adjudication probe (161.36 vs pinned 162.78).
setlocal
call "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvarsall.bat" amd64 >nul
cd /d C:\Users\CarlosRuizMartínez\workspace\Meetily-flash
set CMAKE_GENERATOR=Ninja
set CL=/FS
set CARGO_TARGET_DIR=C:\mf
set MEETIFY_LIVE_DIAG=1
cargo test --release --features vulkan --test closure_gap_probe s14_boundary_signals -- --ignored --nocapture
endlocal
