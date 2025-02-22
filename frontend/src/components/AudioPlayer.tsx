import React from 'react';
import { useAudioPlayer } from '../hooks/useAudioPlayer';

interface AudioPlayerProps {
  audioPath: string | null;
}

export const AudioPlayer: React.FC<AudioPlayerProps> = ({ audioPath }) => {
  const { isPlaying, currentTime, duration, error, isDisabled } = useAudioPlayer(audioPath);

  return (
    <div className="flex items-center space-x-2 p-2 bg-gray-100 rounded-lg">
      <button
        className="px-4 py-2 bg-gray-300 text-gray-500 rounded cursor-not-allowed"
        disabled={true}
        title="Upcoming update"
      >
        {isPlaying ? 'Pause' : 'Play'}
      </button>
      
      <div className="flex-1 h-2 bg-gray-200 rounded">
        <div 
          className="h-full bg-gray-400 rounded" 
          style={{ width: '0%' }}
        />
      </div>
      
      <span className="text-sm text-gray-500">
        {new Date(currentTime * 1000).toISOString().substr(14, 5)} / {new Date(duration * 1000).toISOString().substr(14, 5)}
      </span>
      
      {error && (
        <div className="text-red-500 text-sm">
          Audio playback is currently disabled
        </div>
      )}
    </div>
  );
};