# PowerShell script to start both Whisper server and Python backend with visible output
# This script uses PowerShell's Start-Process to run both servers and show their output

# Set the port for Python backend (default: 5167)
$portPython = 5167
if ($args.Count -gt 0) {
    $portPython = $args[0]
}

# Set the port for Whisper server (default: 8178)
$portWhisper = 8178
if ($args.Count -gt 1) {
    $portWhisper = $args[1]
}

Write-Host "====================================="
Write-Host "Meetily Backend Startup"
Write-Host "====================================="
Write-Host "Python Backend Port: $portPython"
Write-Host "Whisper Server Port: $portWhisper"
Write-Host "====================================="
Write-Host ""

# Kill any existing whisper-server.exe processes
$whisperProcesses = Get-Process -Name "whisper-server" -ErrorAction SilentlyContinue
if ($whisperProcesses) {
    Write-Host "Stopping existing Whisper server processes..."
    $whisperProcesses | ForEach-Object { $_.Kill() }
    Start-Sleep -Seconds 1
}

# Kill any existing python.exe processes
$pythonProcesses = Get-Process -Name "python" -ErrorAction SilentlyContinue
if ($pythonProcesses) {
    Write-Host "Stopping existing Python processes..."
    $pythonProcesses | ForEach-Object { $_.Kill() }
    Start-Sleep -Seconds 1
}

# Check if whisper-server-package exists
if (-not (Test-Path "whisper-server-package")) {
    Write-Host "Error: whisper-server-package directory not found"
    Write-Host "Please run build_whisper.cmd first"
    exit 1
}

# Check if whisper-server.exe exists
if (-not (Test-Path "whisper-server-package\whisper-server.exe")) {
    Write-Host "Error: whisper-server.exe not found"
    Write-Host "Please run build_whisper.cmd first"
    exit 1
}

# Check if models directory exists
if (-not (Test-Path "whisper-server-package\models")) {
    Write-Host "Creating models directory..."
    New-Item -ItemType Directory -Path "whisper-server-package\models" -Force | Out-Null
}

# Define available models
$validModels = @(
    "tiny.en", "tiny", "base.en", "base", "small.en", "small", "medium.en", "medium", 
    "large-v1", "large-v2", "large-v3", "large-v3-turbo", 
    "tiny-q5_1", "tiny.en-q5_1", "tiny-q8_0", 
    "base-q5_1", "base.en-q5_1", "base-q8_0", 
    "small.en-tdrz", "small-q5_1", "small.en-q5_1", "small-q8_0", 
    "medium-q5_0", "medium.en-q5_0", "medium-q8_0", 
    "large-v2-q5_0", "large-v2-q8_0", "large-v3-q5_0", 
    "large-v3-turbo-q5_0", "large-v3-turbo-q8_0"
)

# Define available languages
$validLanguages = @(
    "en", "ar", "bg", "bn", "bs", "ca", "cs", "da", "de", "el", "es", "et", "fa", "fi", "fr", "he", "hi", "hr", "hu", "id", "it", "ja", "ko", "lt", "lv", "mk", "ml", "mr", "ms", "mt", "nl", "no", "pl", "pt", "ro", "ru", "sk", "sl", "so", "sq", "sr", "sv", "ta", "te", "th", "tr", "uk", "ur", "vi", "zh"
)

# Select language
if ($args.Count -gt 2) {
    $language = $args[2]
    if ($validLanguages -notcontains $language) {
        Write-Host "Invalid language: $language"
        Write-Host "Available languages: $($validLanguages -join ", ")"
        exit 1
    }
}

# Get available models
$availableModels = @()
if (Test-Path "whisper-server-package\models") {
    $modelFiles = Get-ChildItem "whisper-server-package\models" -Filter "ggml-*.bin" | ForEach-Object { $_.Name }
    foreach ($file in $modelFiles) {
        if ($file -match "ggml-(.*?)\.bin") {
            $availableModels += $matches[1]
        }
    }
}

# Display available models
Write-Host "====================================="
Write-Host "Model Selection"
Write-Host "====================================="
if ($availableModels.Count -gt 0) {
    Write-Host "Available models in models directory:"
    for ($i = 0; $i -lt $availableModels.Count; $i++) {
        Write-Host "  $($i+1). $($availableModels[$i])"
    }
} else {
    Write-Host "No models found in models directory."
}

Write-Host ""
Write-Host "Default model: small"
Write-Host "Default language: en"
$modelInput = Read-Host "Select a model (1-$($availableModels.Count)) or type model name or press Enter for default (small)"
$languageInput = Read-Host "Select a language (1-$($validLanguages.Count)) or type language name or press Enter for default (en)"

# Process the model selection
$modelName = "small"  # Default model
if (-not [string]::IsNullOrWhiteSpace($modelInput)) {
    if ([int]::TryParse($modelInput, [ref]$null)) {
        $index = [int]$modelInput - 1
        if ($index -ge 0 -and $index -lt $availableModels.Count) {
            $modelName = $availableModels[$index]
        } else {
            Write-Host "Invalid selection. Using default model (small)."
        }
    } else {
        # Check if the input is a valid model name
        if ($validModels -contains $modelInput) {
            $modelName = $modelInput
        } else {
            Write-Host "Invalid model name. Using default model (small)."
        }
    }
}

# Process the language selection
$languageName = "en"  # Default language
if (-not [string]::IsNullOrWhiteSpace($languageInput)) {
    if ([int]::TryParse($languageInput, [ref]$null)) {
        $index = [int]$languageInput - 1
        if ($index -ge 0 -and $index -lt $validLanguages.Count) {
            $languageName = $validLanguages[$index]
        } else {
            Write-Host "Invalid selection. Using default language (en)."
        }
    } else {
        # Check if the input is a valid language name
        if ($validLanguages -contains $languageInput) {
            $languageName = $languageInput
        } else {
            Write-Host "Invalid language name. Using default language (en)."
        }
    }
}

Write-Host "Selected language: $languageName"

# Get port number from user
$portInput = Read-Host "Enter Whisper server port number (default: 8178)"
$portWhisper = 8178
if (-not [string]::IsNullOrWhiteSpace($portInput)) {
    if ([int]::TryParse($portInput, [ref]$null)) {
        $portWhisper = [int]$portInput
    } else {
        Write-Host "Invalid port number. Using default port (8178)."
    }
}

Write-Host "Selected port: $portWhisper"

# Check if the model file exists
$modelFile = "whisper-server-package\models\ggml-$modelName.bin"
if (-not (Test-Path $modelFile)) {
    Write-Host "Model file not found: $modelFile"
    Write-Host "Attempting to download model..."
    
    # Download the model using download-ggml-model.cmd
    $process = Start-Process -FilePath "cmd.exe" -ArgumentList "/c download-ggml-model.cmd $modelName" -NoNewWindow -Wait -PassThru
    if ($process.ExitCode -ne 0) {
        Write-Host "Failed to download model. Using small model instead."
        $modelName = "small"
        
        # Check if small model exists
        if (-not (Test-Path "whisper-server-package\models\ggml-small.bin")) {
            Write-Host "Downloading small model..."
            Start-Process -FilePath "cmd.exe" -ArgumentList "/c download-ggml-model.cmd small" -NoNewWindow -Wait
        }
    } else {
        # Move the model to the models directory if it was downloaded to whisper.cpp/models
        if (Test-Path "whisper.cpp\models\ggml-$modelName.bin") {
            if (-not (Test-Path "whisper-server-package\models")) {
                New-Item -ItemType Directory -Path "whisper-server-package\models" -Force | Out-Null
            }
            Copy-Item "whisper.cpp\models\ggml-$modelName.bin" "whisper-server-package\models\" -Force
            Write-Host "Model copied to whisper-server-package\models directory."
        }
    }
}

Write-Host "====================================="
Write-Host "Starting Meetily Backend"
Write-Host "====================================="
Write-Host "Model: $modelName"
Write-Host "Python Backend Port: $portPython"
Write-Host "Whisper Server Port: $portWhisper"
Write-Host "Language: $languageName"
Write-Host "====================================="
Write-Host ""

# Check if virtual environment exists
if (-not (Test-Path "venv")) {
    Write-Host "Error: Virtual environment not found"
    Write-Host "Please run build_whisper.cmd first"
    exit 1
}

# Check if Python app exists
if (-not (Test-Path "app\main.py")) {
    Write-Host "Error: app\main.py not found"
    Write-Host "Please run build_whisper.cmd first"
    exit 1
}

# Start Whisper server in a new window
Write-Host "Starting Whisper server..."
Start-Process -FilePath "cmd.exe" -ArgumentList "/k cd whisper-server-package && whisper-server.exe --model models\ggml-$modelName.bin --host 127.0.0.1 --port $portWhisper --diarize --print-progress --language $languageName" -WindowStyle Normal

# Wait for Whisper server to start
Write-Host "Waiting for Whisper server to start..."
Start-Sleep -Seconds 5

# Check if Whisper server is running
$whisperRunning = $false
try {
    $whisperProcesses = Get-Process -Name "whisper-server" -ErrorAction Stop
    $whisperRunning = $true
    Write-Host "Whisper server started with PID: $($whisperProcesses.Id)"
} catch {
    Write-Host "Error: Whisper server failed to start"
    exit 1
}

# Start Python backend in a new window
Write-Host "Starting Python backend..."
Start-Process -FilePath "cmd.exe" -ArgumentList "/k call venv\Scripts\activate.bat && set PORT=$portPython && python app\main.py" -WindowStyle Normal

# Wait for Python backend to start
Write-Host "Waiting for Python backend to start..."
Start-Sleep -Seconds 5

# Check if Python backend is running
$pythonRunning = $false
try {
    $pythonProcesses = Get-Process -Name "python" -ErrorAction Stop
    $pythonRunning = $true
    Write-Host "Python backend started with PID: $($pythonProcesses.Id)"
} catch {
    Write-Host "Error: Python backend failed to start"
    exit 1
}

# Check if services are listening on their ports
Write-Host "Checking if services are listening on their ports..."
$whisperListening = $false
$pythonListening = $false

# Wait a bit longer for services to start listening
Start-Sleep -Seconds 5

# Check Whisper server port
$netstatWhisper = netstat -ano | Select-String -Pattern ":$portWhisper.*LISTENING"
if ($netstatWhisper) {
    $whisperListening = $true
    Write-Host "Whisper server is listening on port $portWhisper"
} else {
    Write-Host "Warning: Whisper server is not listening on port $portWhisper"
}

# Check Python backend port
$netstatPython = netstat -ano | Select-String -Pattern ":$portPython.*LISTENING"
if ($netstatPython) {
    $pythonListening = $true
    Write-Host "Python backend is listening on port $portPython"
} else {
    Write-Host "Warning: Python backend is not listening on port $portPython"
}

# Final status
Write-Host ""
Write-Host "====================================="
Write-Host "Backend Status"
Write-Host "====================================="
Write-Host "Whisper Server: $(if ($whisperRunning) { "RUNNING" } else { "NOT RUNNING" })"
Write-Host "Whisper Server Port: $(if ($whisperListening) { "LISTENING on $portWhisper" } else { "NOT LISTENING on $portWhisper" })"
Write-Host "Python Backend: $(if ($pythonRunning) { "RUNNING" } else { "NOT RUNNING" })"
Write-Host "Python Backend Port: $(if ($pythonListening) { "LISTENING on $portPython" } else { "NOT LISTENING on $portPython" })"
Write-Host ""
Write-Host "The backend services are now running in separate windows."
Write-Host "You can close those windows to stop the services."
Write-Host "====================================="
