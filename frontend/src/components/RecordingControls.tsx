'use client';

import { invoke } from '@tauri-apps/api/core';
import { appDataDir } from '@tauri-apps/api/path';
import { useCallback, useEffect, useState, useRef } from 'react';
import { Play, Pause, Square, Mic } from 'lucide-react';
import { useAudioPlayer } from '@/hooks/useAudioPlayer';
import { ProcessRequest, SummaryResponse } from '@/types/summary';

interface RecordingControlsProps {
  isRecording: boolean;
  barHeights: string[];
  onRecordingStop: () => void;
  onRecordingStart: () => void;
  onTranscriptReceived: (summary: SummaryResponse) => void;
}

export const RecordingControls: React.FC<RecordingControlsProps> = ({
  isRecording,
  barHeights,
  onRecordingStop,
  onRecordingStart,
  onTranscriptReceived,
}) => {
  const [showPlayback, setShowPlayback] = useState(false);
  const [recordingPath, setRecordingPath] = useState<string | null>(null);
  const [transcript, setTranscript] = useState<string>('');
  const [isProcessing, setIsProcessing] = useState(false);
  const [isStarting, setIsStarting] = useState(false);
  const [isStopping, setIsStopping] = useState(false);
  const [stopCountdown, setStopCountdown] = useState(5);
  const countdownInterval = useRef<NodeJS.Timeout | null>(null);
  const stopTimeoutRef = useRef<{ stop: () => void } | null>(null);
  const MIN_RECORDING_DURATION = 2000; // 2 seconds minimum recording time
  const { isPlaying, currentTime, duration, play, pause, seek } = useAudioPlayer(recordingPath);

  const formatTime = (time: number) => {
    const minutes = Math.floor(time / 60);
    const seconds = Math.floor(time % 60);
    return `${minutes}:${seconds.toString().padStart(2, '0')}`;
  };

  useEffect(() => {
    const checkTauri = async () => {
      try {
        const result = await invoke('is_recording');
        console.log('Tauri is initialized and ready, is_recording result:', result);
      } catch (error) {
        console.error('Tauri initialization error:', error);
        alert('Failed to initialize recording. Please check the console for details.');
      }
    };
    checkTauri();
  }, []);

  const handleStartRecording = useCallback(async () => {
    if (isStarting) return;
    console.log('Starting recording...');
    setIsStarting(true);
    setShowPlayback(false);
    setTranscript(''); // Clear any previous transcript
    
    try {
      await invoke('start_recording');
      console.log('Recording started successfully');
      setIsProcessing(false);
      onRecordingStart();
    } catch (error) {
      console.error('Failed to start recording:', error);
      alert('Failed to start recording. Please check the console for details.');
    } finally {
      setIsStarting(false);
    }
  }, [onRecordingStart, isStarting]);

  const stopRecordingAction = useCallback(async () => {
    console.log('Executing stop recording...');
    try {
      setIsProcessing(true);
      const dataDir = await appDataDir();
      const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
      const savePath = `${dataDir}/recording-${timestamp}.wav`;
      
      console.log('Saving recording to:', savePath);
      const result = await invoke('stop_recording', { 
        args: {
          save_path: savePath
        }
      });
      
      setRecordingPath(savePath);
      setShowPlayback(true);
      setIsProcessing(false);
      onRecordingStop();
    } catch (error) {
      console.error('Failed to stop recording:', error);
      if (error instanceof Error) {
        console.error('Error details:', {
          message: error.message,
          name: error.name,
          stack: error.stack,
        });
        if (error.message.includes('No recording in progress')) {
          return;
        }
      } else if (typeof error === 'string' && error.includes('No recording in progress')) {
        return;
      } else if (error && typeof error === 'object' && 'toString' in error) {
        if (error.toString().includes('No recording in progress')) {
          return;
        }
      }
      setIsProcessing(false);
      onRecordingStop();
    } finally {
      setIsStopping(false);
    }
  }, [onRecordingStop]);

  const handleStopRecording = useCallback(async () => {
    if (!isRecording || isStarting || isStopping) return;
    
    console.log('Starting stop countdown...');
    setIsStopping(true);
    setStopCountdown(5);

    // Clear any existing intervals
    if (countdownInterval.current) {
      clearInterval(countdownInterval.current);
      countdownInterval.current = null;
    }

    // Create a controller for the stop action
    const controller = {
      stop: () => {
        if (countdownInterval.current) {
          clearInterval(countdownInterval.current);
          countdownInterval.current = null;
        }
        setIsStopping(false);
        setStopCountdown(5);
      }
    };
    stopTimeoutRef.current = controller;

    // Start countdown
    countdownInterval.current = setInterval(() => {
      setStopCountdown(prev => {
        if (prev <= 1) {
          // Clear interval first
          if (countdownInterval.current) {
            clearInterval(countdownInterval.current);
            countdownInterval.current = null;
          }
          // Schedule stop action
          stopRecordingAction();
          return 0;
        }
        return prev - 1;
      });
    }, 1000);
  }, [isRecording, isStarting, isStopping, stopRecordingAction]);

  const cancelStopRecording = useCallback(() => {
    if (stopTimeoutRef.current) {
      stopTimeoutRef.current.stop();
      stopTimeoutRef.current = null;
    }
  }, []);

  useEffect(() => {
    if (recordingPath) {
      console.log('Checking if file exists:', recordingPath);
      invoke('read_audio_file', { filePath: recordingPath })
        .then(() => console.log('Audio file exists and is readable'))
        .catch(error => console.error('Error reading audio file:', error));
    }
  }, [recordingPath]);

  useEffect(() => {
    return () => {
      if (countdownInterval.current) clearInterval(countdownInterval.current);
      if (stopTimeoutRef.current) stopTimeoutRef.current.stop();
    };
  }, []);

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
      {isProcessing ? (
        <div className="flex items-center space-x-2">
          <div className="animate-spin rounded-full h-5 w-5 border-b-2 border-gray-900"></div>
          <span className="text-sm text-gray-600">Processing recording...</span>
        </div>
      ) : (
        <>
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
                onClick={isRecording ? 
                  (isStopping ? cancelStopRecording : handleStopRecording) : 
                  handleStartRecording}
                disabled={isStarting || isProcessing}
                className={`w-12 h-12 flex items-center justify-center ${
                  isStarting || isProcessing ? 'bg-gray-400' : 'bg-red-500 hover:bg-red-600'
                } rounded-full text-white transition-colors relative`}
              >
                {isRecording ? (
                  <>
                    <Square size={20} />
                    {isStopping && (
                      <div className="absolute -top-8 text-red-500 font-medium">
                        {stopCountdown > 0 ? `${stopCountdown}s` : 'Stopping...'}
                      </div>
                    )}
                  </>
                ) : (
                  <Mic size={20} />
                )}
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
        </>
      )}
    </div>
  );
};