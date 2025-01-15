#!/bin/bash

# Exit on error
set -e

# Kill any existing whisper-server processes
echo "Checking for existing whisper servers..."
pkill -f "whisper-server" || true
pkill -f "language_server_macos_arm" || true
sleep 1  # Give processes time to terminate

# Clean up previous builds
echo "Cleaning up previous builds..."
# rm -rf target/
# rm -rf src-tauri/target
rm -rf src-tauri/gen

# Clean up npm, pnp and next
echo "Cleaning up npm, pnp and next..."
rm -rf node_modules
rm -rf .next
rm -rf .pnp.cjs

echo "Installing dependencies..."
pnpm install

# Configuration
PACKAGE_NAME="whisper-server-package"
MODEL_DIR="$PACKAGE_NAME/models"

# Check for existing model
echo "Checking for Whisper model..."
EXISTING_MODEL=$(find "$MODEL_DIR" -name "ggml-*.bin" -type f -print -quit)

echo "EXISTING_MODEL: $EXISTING_MODEL"
# if [ -n "$EXISTING_MODEL" ]; then
#     MODEL_NAME=$(basename "$EXISTING_MODEL")
#     echo "Found existing model: $MODEL_NAME"
# else
#     MODEL_NAME="ggml-small.bin"
#     echo "No model found. Will use: $MODEL_NAME"
# fi

# Whisper models
models="tiny
tiny.en
tiny-q5_1
tiny.en-q5_1
tiny-q8_0
base
base.en
base-q5_1
base.en-q5_1
base-q8_0
small
small.en
small.en-tdrz
small-q5_1
small.en-q5_1
small-q8_0
medium
medium.en
medium-q5_0
medium.en-q5_0
medium-q8_0
large-v1
large-v2
large-v2-q5_0
large-v2-q8_0
large-v3
large-v3-q5_0
large-v3-turbo
large-v3-turbo-q5_0
large-v3-turbo-q8_0"

 MODEL_SHORT_NAME="large-v3"
# MODEL_SHORT_NAME="large-v3-turbo-q5_0"

 MODEL_NAME="ggml-$MODEL_SHORT_NAME.bin"

 # Check if the modelname exists in directory
 if [ -f "$MODEL_DIR/$MODEL_NAME" ]; then
     echo "Model file exists: $MODEL_DIR/$MODEL_NAME"
 else
     echo "Model file does not exist: $MODEL_DIR/$MODEL_NAME"
     echo "Trying to download model..."
     ./whisper-server-package/models/download-ggml-model.sh $MODEL_SHORT_NAME

    #  exit 1
 fi

# Start the whisper server
echo "Starting Whisper server..."
cd "$PACKAGE_NAME"
./run-server.sh --model "models/$MODEL_NAME" &
WHISPER_PID=$!
cd ..

# Cleanup function
cleanup() {
    echo "Cleaning up... "
    if [ -n "$WHISPER_PID" ]; then
        echo "Stopping Whisper server and related processes..."
        # Kill the main process
        kill -9 $WHISPER_PID 2>/dev/null || true
        # Kill any remaining whisper-server processes
        pkill -9 -f "whisper-server" 2>/dev/null || true
        # Kill language server process
        # pkill -9 -f "language_server_macos_arm" 2>/dev/null || true
    fi
    exit 0
}

# Set up trap for cleanup
trap cleanup EXIT INT TERM

# Give the server a moment to start
sleep 2

echo "Building Tauri app..."
pnpm run tauri dev
