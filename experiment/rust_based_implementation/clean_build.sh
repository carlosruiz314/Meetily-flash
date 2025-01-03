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
