#!/bin/bash

# Default configuration
HOST="127.0.0.1"
PORT="8178"
MODEL="models/ggml-large-v3.bin"

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --host)
            HOST="$2"
            shift 2
            ;;
        --port)
            PORT="$2"
            shift 2
            ;;
        --model)
            MODEL="$2"
            shift 2
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Run the server
./whisper-server \
    --model "$MODEL" \
    --host "$HOST" \
    --port "$PORT" \
    --diarize \
    --print-progress

