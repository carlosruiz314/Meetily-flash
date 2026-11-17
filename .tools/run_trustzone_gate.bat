@echo off
rem Trust-zone split fix: lib tests + ear-truth gate (synthetic twin removed, engine-boundary 6.2).
setlocal
call "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvarsall.bat" amd64 >nul
cd /d C:\Users\devuser\workspace\Meetily-flash
set CMAKE_GENERATOR=Ninja
set CL=/FS
set CARGO_TARGET_DIR=C:\mf
set MEETIFY_LIVE_DIAG=1

echo === lib tests ===
cargo test --release --features vulkan --lib 2>&1
if errorlevel 1 goto :fail
echo === ear gate ===
cargo test --release --features vulkan --test ear_truth_gate -- --ignored --nocapture 2>&1
if errorlevel 1 goto :fail
echo === ALL STAGES PASSED ===
goto :eof
:fail
echo === STAGE FAILED ===
exit /b 1
