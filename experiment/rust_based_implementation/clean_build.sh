#!/bin/bash

# Exit on error
set -e

# Clean up previous builds
echo "Cleaning up previous builds..."
# rm -rf target/
# rm -rf src-tauri/target

echo "Installing dependencies..."
pnpm install

# Configuration
PACKAGE_NAME="whisper-server-package"
MODEL_DIR="$PACKAGE_NAME/models"

# Check for existing model
echo "Checking for Whisper model..."
EXISTING_MODEL=$(find "$MODEL_DIR" -name "ggml-*.bin" -type f -print -quit)

if [ -n "$EXISTING_MODEL" ]; then
    MODEL_NAME=$(basename "$EXISTING_MODEL")
    echo "Found existing model: $MODEL_NAME"
else
    MODEL_NAME="ggml-small.en.bin"
    echo "No model found. Will use: $MODEL_NAME"
fi

# Start the whisper server
echo "Starting Whisper server..."
cd "$PACKAGE_NAME"
./run-server.sh --model "models/$MODEL_NAME" &
cd ..

# Give the server a moment to start
sleep 2

echo "Building Tauri app..."
pnpm run tauri dev
