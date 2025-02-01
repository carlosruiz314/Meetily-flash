#!/bin/bash

# Exit on error
set -e






# Kill any existing whisper-server processes
echo "Checking for existing whisper servers..."
pkill -f "whisper-server" || true
sleep 1  # Give processes time to terminate

# Check and kill if backend app in port 5167 is running
echo "Checking for existing backend app..."
PORT=5167
if lsof -i :$PORT | grep -q LISTEN; then
    # Ask user to confirm
    read -p "Backend app is running on port $PORT. Kill it? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Aborting..."
        exit 1
    fi

    echo "Backend app is running on port $PORT. Killing..."
    kill -9 $(lsof -t -i :$PORT)
    sleep 1  # Give processes time to terminate
fi

# Configuration
PACKAGE_NAME="whisper-server-package"
MODEL_DIR="$PACKAGE_NAME/models"

# If model directory does not exist, create it
if [ ! -d "$MODEL_DIR" ]; then
    echo "Creating model directory: $MODEL_DIR"
    mkdir -p "$MODEL_DIR"
fi

# Check for existing model
echo "Checking for Whisper models..."

# There will be multiple models in the directory
EXISTING_MODELS=$(find "$MODEL_DIR" -name "ggml-*.bin" -type f)

echo "Existing models: $EXISTING_MODELS"

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

# Ask user which model to use if the argument is not provided
if [ -z "$1" ]; then
    # Let user interactively select a model name
    echo "Available models: $models"
    read -p "Enter a model name (e.g. small): " MODEL_SHORT_NAME
else
    MODEL_SHORT_NAME=$1
fi

# Check if the model is valid
if ! echo "$models" | grep -qw "$MODEL_SHORT_NAME"; then
    echo "Invalid model: $MODEL_SHORT_NAME"
    exit 1
fi

MODEL_NAME="ggml-$MODEL_SHORT_NAME.bin"

# Check if the modelname exists in directory
if [ -f "$MODEL_DIR/$MODEL_NAME" ]; then
    echo "Model file exists: $MODEL_DIR/$MODEL_NAME"
else
    echo "Model file does not exist: $MODEL_DIR/$MODEL_NAME"
    echo "Trying to download model..."
    ./whisper-server-package/models/download-ggml-model.sh $MODEL_SHORT_NAME
fi

# Start the whisper server in background
cd "$PACKAGE_NAME"
./run-server.sh --model "models/$MODEL_NAME" &
WHISPER_PID=$!
cd ..

sleep 2

# Start the Python backend in background
if [ -z "$VIRTUAL_ENV" ]; then
    echo "Activating venv..."
    source venv/bin/activate
fi

echo "Starting Python backend..."
python app/main.py &
PYTHON_PID=$!

# Cleanup function
cleanup() {
    echo "Cleaning up..."
    if [ -n "$WHISPER_PID" ]; then
        echo "Stopping Whisper server..."
        kill -9 $WHISPER_PID 2>/dev/null || true
        pkill -9 -f "whisper-server" 2>/dev/null || true
    fi
    if [ -n "$PYTHON_PID" ]; then
        echo "Stopping Python backend..."
        kill -9 $PYTHON_PID 2>/dev/null || true
    fi
    exit 0
}

# Set up trap for cleanup
trap cleanup EXIT INT TERM

# Keep the script running and wait for both processes
wait $WHISPER_PID $PYTHON_PID
