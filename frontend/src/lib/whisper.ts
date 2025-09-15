// Types for whisper-rs integration
export interface ModelInfo {
  name: string;
  path: string;
  size_mb: number;
  accuracy: ModelAccuracy;
  speed: ProcessingSpeed;
  status: ModelStatus;
  description?: string;
}

export type ModelAccuracy = 'High' | 'Good' | 'Decent';
export type ProcessingSpeed = 'Slow' | 'Medium' | 'Fast';

export type ModelStatus = 
  | 'Available' 
  | 'Missing' 
  | { Downloading: number } 
  | { Error: string };

export interface ModelDownloadProgress {
  modelName: string;
  progress: number;
  totalBytes: number;
  downloadedBytes: number;
  speed: string;
}

export interface WhisperEngineState {
  currentModel: string | null;
  availableModels: ModelInfo[];
  isLoading: boolean;
  error: string | null;
}

// Tauri command interfaces
export interface DownloadModelRequest {
  modelName: string;
}

export interface SwitchModelRequest {
  modelName: string;
}

export interface TranscribeAudioRequest {
  audioData: number[];
  sampleRate: number;
}

// Model configuration for different use cases
export const MODEL_CONFIGS: Record<string, Partial<ModelInfo>> = {
  'large-v3': {
    description: 'Highest accuracy, best for important meetings. Slower processing.',
    size_mb: 3000,
    accuracy: 'High',
    speed: 'Slow'
  },
  'medium': {
    description: 'Balanced accuracy and speed. Good for most use cases.',
    size_mb: 1400,
    accuracy: 'Good',
    speed: 'Medium'
  },
  'small': {
    description: 'Fast processing with good quality. Great for quick transcription.',
    size_mb: 465,
    accuracy: 'Decent',
    speed: 'Fast'
  }
};

// Helper functions
export function getModelIcon(accuracy: ModelAccuracy): string {
  switch (accuracy) {
    case 'High': return '🔥';
    case 'Good': return '⚡';
    case 'Decent': return '🚀';
    default: return '📊';
  }
}

export function getStatusColor(status: ModelStatus): string {
  if (status === 'Available') return 'green';
  if (status === 'Missing') return 'gray';
  if (typeof status === 'object' && 'Downloading' in status) return 'blue';
  if (typeof status === 'object' && 'Error' in status) return 'red';
  return 'gray';
}

export function formatFileSize(sizeMb: number): string {
  if (sizeMb >= 1000) {
    return `${(sizeMb / 1000).toFixed(1)}GB`;
  }
  return `${sizeMb}MB`;
}

export function getRecommendedModel(systemSpecs?: { ram: number; cores: number }): string {
  if (!systemSpecs) return 'medium'; // Default fallback
  
  if (systemSpecs.ram >= 8000 && systemSpecs.cores >= 8) {
    return 'large-v3'; // High-end system
  } else if (systemSpecs.ram >= 4000 && systemSpecs.cores >= 4) {
    return 'medium'; // Mid-range system
  }
  return 'small'; // Lower-spec system
}

// Tauri command wrappers for whisper-rs backend
import { invoke } from '@tauri-apps/api/core';

export class WhisperAPI {
  static async init(): Promise<void> {
    await invoke('whisper_init');
  }
  
  static async getAvailableModels(): Promise<ModelInfo[]> {
    return await invoke('whisper_get_available_models');
  }
  
  static async loadModel(modelName: string): Promise<void> {
    await invoke('whisper_load_model', { modelName });
  }
  
  static async getCurrentModel(): Promise<string | null> {
    return await invoke('whisper_get_current_model');
  }
  
  static async isModelLoaded(): Promise<boolean> {
    return await invoke('whisper_is_model_loaded');
  }
  
  static async transcribeAudio(audioData: number[]): Promise<string> {
    return await invoke('whisper_transcribe_audio', { audioData });
  }
  
  static async getModelsDirectory(): Promise<string> {
    return await invoke('whisper_get_models_directory');
  }
  
  static async downloadModel(modelName: string): Promise<void> {
    await invoke('whisper_download_model', { modelName });
  }
  
  static async cancelDownload(modelName: string): Promise<void> {
    await invoke('whisper_cancel_download', { modelName });
  }
}
