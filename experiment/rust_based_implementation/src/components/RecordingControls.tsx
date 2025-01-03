'use client';

import { invoke } from '@tauri-apps/api/core';
import { appDataDir } from '@tauri-apps/api/path';
import { useCallback, useEffect, useState } from 'react';
import { Play, Pause, Square, Mic } from 'lucide-react';
import { useAudioPlayer } from '@/hooks/useAudioPlayer';

interface RecordingControlsProps {
  isRecording: boolean;
  barHeights: string[];
  onRecordingStop: () => void;
  onRecordingStart: () => void;
}

export const RecordingControls: React.FC<RecordingControlsProps> = ({
  isRecording,
  barHeights,
  onRecordingStop,
  onRecordingStart,
}) => {
  const [showPlayback, setShowPlayback] = useState(false);
  const [recordingPath, setRecordingPath] = useState<string | null>(null);
  const { isPlaying, currentTime, duration, play, pause, seek } = useAudioPlayer(recordingPath);

  const formatTime = (time: number) => {
    const minutes = Math.floor(time / 60);
    const seconds = Math.floor(time % 60);
    return `${minutes}:${seconds.toString().padStart(2, '0')}`;
  };

  useEffect(() => {
    const checkTauri = async () => {
      try {
        await invoke('is_recording');
        console.log('Tauri is initialized and ready');
      } catch (error) {
        console.error('Tauri initialization error:', error);
      }
    };
    checkTauri();
  }, []);

  const handleStartRecording = useCallback(async () => {
    console.log('Starting recording...');
    setShowPlayback(false);
    try {
      await invoke('start_recording');
      console.log('Recording started successfully');
      onRecordingStart();
    } catch (error) {
      console.error('Failed to start recording:', error);
      if (error instanceof Error) {
        console.error('Error details:', {
          message: error.message,
          name: error.name,
          stack: error.stack,
        });
      }
    }
  }, [onRecordingStart]);

  const handleStopRecording = useCallback(async () => {
    console.log('Stopping recording...');
    try {
      const dataDir = await appDataDir();
      const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
      const savePath = `${dataDir}/recording-${timestamp}.wav`;
      
      console.log('Saving recording to:', savePath);
      await invoke('stop_recording', { 
        args: { save_path: savePath }
      });
      console.log('Recording stopped and saved successfully');
      
      // Set recording path and show playback
      console.log('Setting recording path:', savePath);
      setRecordingPath(savePath);
      setShowPlayback(true);
      onRecordingStop();
    } catch (error) {
      console.error('Failed to stop recording:', error);
      if (error instanceof Error) {
        console.error('Error details:', {
          message: error.message,
          name: error.name,
          stack: error.stack,
        });
      }
      // Don't call onRecordingStop if there was no recording in progress
      if (error.toString().includes('No recording in progress')) {
        return;
      }
      onRecordingStop();
    }
  }, [onRecordingStop]);

  useEffect(() => {
    if (recordingPath) {
      console.log('Checking if file exists:', recordingPath);
      invoke('read_audio_file', { filePath: recordingPath })
        .then(() => console.log('Audio file exists and is readable'))
        .catch(error => console.error('Error reading audio file:', error));
    }
  }, [recordingPath]);

  const togglePlayback = useCallback(async () => {
    console.log('Toggle playback:', {
      isPlaying,
      recordingPath,
      showPlayback
    });
    
    if (isPlaying) {
      pause();
    } else {
      try {
        // Try to read the file first to ensure it exists
        await invoke('read_audio_file', { filePath: recordingPath });
        play();
      } catch (error) {
        console.error('Error reading audio file before playback:', error);
      }
    }
  }, [isPlaying, play, pause, recordingPath]);

  const progress = duration > 0 ? (currentTime / duration) * 100 : 0;

  return (
    <div className="flex items-center space-x-2 bg-white rounded-full shadow-lg px-4 py-2">
      {showPlayback ? (
        <>
          <button
            onClick={handleStartRecording}
            className="w-10 h-10 flex items-center justify-center bg-red-500 rounded-full text-white hover:bg-red-600 transition-colors"
          >
            <Mic size={16} />
          </button>

          <div className="w-px h-6 bg-gray-200 mx-1" />

          <div className="flex items-center space-x-1 mx-2">
            <div className="text-sm text-gray-600 min-w-[40px]">
              {formatTime(currentTime)}
            </div>
            <div 
              className="relative w-24 h-1 bg-gray-200 rounded-full cursor-pointer hover:bg-gray-300 transition-colors"
              onClick={async (e) => {
                try {
                  const rect = e.currentTarget.getBoundingClientRect();
                  const x = e.clientX - rect.left;
                  const percent = Math.max(0, Math.min(1, x / rect.width));
                  const newTime = duration * percent;
                  console.log('Timeline clicked:', { percent, newTime });
                  await seek(newTime);
                } catch (error) {
                  console.error('Error seeking:', error);
                }
              }}
            >
              <div 
                className="absolute h-full bg-blue-500 rounded-full" 
                style={{ width: `${progress}%` }}
              />
            </div>
            <div className="text-sm text-gray-600 min-w-[40px]">
              {formatTime(duration)}
            </div>
          </div>

          <button 
            onClick={togglePlayback}
            className="w-10 h-10 flex items-center justify-center bg-blue-500 rounded-full text-white hover:bg-blue-600 transition-colors"
          >
            {isPlaying ? <Pause size={16} /> : <Play size={16} />}
          </button>
        </>
      ) : (
        <>
          <button
            onClick={isRecording ? handleStopRecording : handleStartRecording}
            className="w-12 h-12 flex items-center justify-center bg-red-500 rounded-full text-white hover:bg-red-600 transition-colors"
          >
            {isRecording ? <Square size={20} /> : <Mic size={20} />}
          </button>

          <div className="flex items-center space-x-1 mx-4">
            {barHeights.map((height, index) => (
              <div
                key={index}
                className="w-1 bg-red-500 rounded-full transition-all duration-200"
                style={{
                  height: isRecording ? height : '4px',
                }}
              />
            ))}
          </div>
        </>
      )}
    </div>
  );
};