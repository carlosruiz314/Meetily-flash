#!/bin/bash

# Exit on error
set -e

git submodule update --init --recursive

cd whisper.cpp

cp -r ../whisper-custom/server/* "examples/server/"

ls "examples/server/"

# Build Whisper server
make -j4

# Configuration
PACKAGE_NAME="whisper-server-package"
MODEL_NAME="ggml-small.bin"


# Check if package directory exists
# if [ -d "$PACKAGE_NAME" ]; then
#     echo "Package directory already exists: $PACKAGE_NAME"
#     exit 1
# fi

MODEL_DIR="$PACKAGE_NAME/models"




# Copy server binary
cp build/bin/whisper-server "$PACKAGE_NAME/"

# Copy model file

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
    ./models/download-ggml-model.sh $MODEL_SHORT_NAME
    # Move model to models directory
    mv "./models/$MODEL_NAME" "$MODEL_DIR/"
fi

# Create run script
cat > "$PACKAGE_NAME/run-server.sh" << 'EOL'
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


EOL

echo "Make script executable: $PACKAGE_NAME/run-server.sh"
# Make run script executable
chmod +x "$PACKAGE_NAME/run-server.sh"

ls

# Check if package directory already exists
if [ -d "../$PACKAGE_NAME" ]; then
    ls ..
    echo "Package directory already exists: ../$PACKAGE_NAME"
    ls "../$PACKAGE_NAME"
else
    echo "Creating package directory: ../$PACKAGE_NAME"
    ls ..
    mkdir "../$PACKAGE_NAME"
    ls "../$PACKAGE_NAME"
fi

# Move whisper-server package out of whisper.cpp to ../PACKAGE_NAME

# If package directory already exists outside whisper.cpp, copy just whisper-server and model to it. Replace
# the contents of the directory with the new files
if [ -d "../$PACKAGE_NAME" ]; then
    cp -r "$PACKAGE_NAME/"* "../$PACKAGE_NAME"
    
else
   
   echo "Copying whisper-server and model to ../$PACKAGE_NAME"
    cp "$MODEL_DIR/$MODEL_NAME" "../$PACKAGE_NAME/models/"
    cp "$PACKAGE_NAME/run-server.sh" "../$PACKAGE_NAME"
    cp -r "$PACKAGE_NAME/public" "../$PACKAGE_NAME"
    cp "$PACKAGE_NAME/whisper-server" "../$PACKAGE_NAME"
    # rm -r "$PACKAGE_NAME"
fi



echo "Done!"