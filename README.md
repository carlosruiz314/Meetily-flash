# Meeting Minutes - AI-Powered Meeting Assistant

An AI-powered meeting assistant that captures live meeting audio, transcribes it in real-time, and generates summaries while ensuring user privacy.

## Features

- Automatic meeting detection (Zoom, Google Meet, Teams)
- Live audio transcription using OpenAI's Whisper
- Real-time display of transcription
- Post-meeting summarization
- Local processing for privacy
- Export to Markdown/PDF

## Prerequisites

- Node.js >= 18
- Python >= 3.9
- Virtual audio driver (BlackHole recommended for macOS)

## Setup

### Backend Setup

1. Create a Python virtual environment:
```bash
python -m venv venv
source venv/bin/activate  # On Windows: .\venv\Scripts\activate
```

2. Install dependencies:
```bash
cd backend
pip install -r requirements.txt
```

3. Start the backend server:
```bash
cd app
uvicorn main:app --reload
```

### Frontend Setup

1. Install dependencies:
```bash
cd frontend
npm install
```

2. Start the development server:
```bash
npm run dev
```

3. In a new terminal, start the Electron app:
```bash
npm start
```

## Development

- Frontend: Next.js + Electron application in `frontend/`
- Backend: FastAPI application in `backend/`
- Real-time communication via WebSocket
- Local SQLite database for storing meeting data

## License

MIT
