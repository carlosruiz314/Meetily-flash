import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { motion, AnimatePresence } from 'framer-motion';
import { toast } from 'sonner';
import {
  AlertCircle,
  CheckCircle2,
  Download,
  ExternalLink,
  Loader2,
  Package,
  HardDrive,
  Wifi,
  Play,
  XCircle
} from 'lucide-react';
import { Button } from './ui/button';
import { Alert, AlertDescription, AlertTitle } from './ui/alert';
import { Progress } from './ui/progress';

interface LfmModelManagerProps {
  selectedModel?: string;
  onModelSelect?: (modelName: string) => void;
  className?: string;
  autoSave?: boolean;
}

type SetupStage =
  | 'not_started'
  | 'checking_python'
  | 'creating_venv'
  | 'installing_dependencies'
  | 'downloading_model'
  | 'verifying'
  | 'complete'
  | 'failed';

interface SetupProgress {
  stage: SetupStage;
  progress: number;
  message: string;
  details?: string;
}

type LfmStatus = 'not_setup' | 'setup_complete' | 'loading' | 'loaded' | 'error';

export function LfmModelManager({
  selectedModel,
  onModelSelect,
  className = '',
  autoSave = false
}: LfmModelManagerProps) {
  const [status, setStatus] = useState<LfmStatus>('not_setup');
  const [loading, setLoading] = useState(true);
  const [isSettingUp, setIsSettingUp] = useState(false);
  const [setupProgress, setSetupProgress] = useState<SetupProgress | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Check setup status on mount
  useEffect(() => {
    checkSetupStatus();
  }, []);

  // Listen for setup progress events
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;

    const setupListener = async () => {
      unlisten = await listen<SetupProgress>('lfm-setup-progress', (event) => {
        setSetupProgress(event.payload);

        if (event.payload.stage === 'complete') {
          setIsSettingUp(false);
          setStatus('setup_complete');
          toast.success('LFM2.5-Audio setup complete!', {
            description: 'You can now use Liquid AI for transcription',
            duration: 5000
          });
        } else if (event.payload.stage === 'failed') {
          setIsSettingUp(false);
          setStatus('error');
          setError(event.payload.details || event.payload.message);
        }
      });
    };

    setupListener();

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  const checkSetupStatus = async () => {
    setLoading(true);
    try {
      const isSetupComplete = await invoke<boolean>('lfm_is_setup_complete');

      if (isSetupComplete) {
        // Check if model is loaded
        const isModelLoaded = await invoke<boolean>('lfm_is_model_loaded');
        setStatus(isModelLoaded ? 'loaded' : 'setup_complete');
      } else {
        setStatus('not_setup');
      }
    } catch (err) {
      console.error('Failed to check LFM setup status:', err);
      setStatus('not_setup');
    } finally {
      setLoading(false);
    }
  };

  const startSetup = async () => {
    setIsSettingUp(true);
    setError(null);
    setSetupProgress({
      stage: 'not_started',
      progress: 0,
      message: 'Starting setup...'
    });

    try {
      toast.info('Starting LFM2.5-Audio setup...', {
        description: 'This will take 5-10 minutes',
        duration: 10000
      });

      await invoke('lfm_run_setup');
    } catch (err) {
      console.error('Setup failed:', err);
      setIsSettingUp(false);
      setError(err instanceof Error ? err.message : String(err));
      setStatus('error');
      toast.error('Setup failed', {
        description: err instanceof Error ? err.message : String(err),
        duration: 6000
      });
    }
  };

  const loadModel = async () => {
    setStatus('loading');
    setError(null);

    try {
      toast.info('Loading LFM2.5-Audio model...', {
        description: 'Initializing Liquid AI engine',
        duration: 5000
      });

      await invoke('lfm_load_model', { modelId: null });

      setStatus('loaded');
      toast.success('LFM2.5-Audio ready!', {
        description: 'Liquid AI transcription model loaded',
        duration: 4000
      });

      // Notify parent
      if (onModelSelect) {
        onModelSelect('LiquidAI/LFM2.5-Audio-1.5B');
      }

      // Auto-save selection
      if (autoSave) {
        await invoke('api_save_transcript_config', {
          provider: 'lfm',
          model: 'LiquidAI/LFM2.5-Audio-1.5B',
          apiKey: null
        });
      }
    } catch (err) {
      console.error('Failed to load LFM model:', err);
      const errorMsg = err instanceof Error ? err.message : 'Failed to load model';
      setError(errorMsg);
      setStatus('error');
      toast.error('Failed to load LFM model', {
        description: errorMsg,
        duration: 6000
      });
    }
  };

  const unloadModel = async () => {
    try {
      await invoke('lfm_unload_model');
      setStatus('setup_complete');
      toast.info('LFM model unloaded', {
        description: 'Memory freed',
        duration: 3000
      });
    } catch (err) {
      console.error('Failed to unload model:', err);
    }
  };

  // Get stage-specific icon and color
  const getStageInfo = (stage: SetupStage) => {
    switch (stage) {
      case 'checking_python':
        return { icon: Package, color: 'text-blue-500', label: 'Checking Python' };
      case 'creating_venv':
        return { icon: HardDrive, color: 'text-purple-500', label: 'Creating Environment' };
      case 'installing_dependencies':
        return { icon: Download, color: 'text-orange-500', label: 'Installing Dependencies' };
      case 'downloading_model':
        return { icon: Wifi, color: 'text-green-500', label: 'Downloading Model' };
      case 'verifying':
        return { icon: CheckCircle2, color: 'text-emerald-500', label: 'Verifying' };
      case 'complete':
        return { icon: CheckCircle2, color: 'text-green-600', label: 'Complete' };
      case 'failed':
        return { icon: XCircle, color: 'text-red-500', label: 'Failed' };
      default:
        return { icon: Loader2, color: 'text-gray-500', label: 'Preparing' };
    }
  };

  if (loading) {
    return (
      <div className={`space-y-3 ${className}`}>
        <div className="animate-pulse space-y-3">
          <div className="h-24 bg-gray-100 rounded-lg"></div>
        </div>
      </div>
    );
  }

  // Not set up - show setup UI
  if (status === 'not_setup' && !isSettingUp) {
    return (
      <div className={`space-y-4 ${className}`}>
        <motion.div
          initial={{ opacity: 0, y: 5 }}
          animate={{ opacity: 1, y: 0 }}
          className="relative rounded-lg border-2 border-purple-200 bg-gradient-to-br from-purple-50 to-white p-5"
        >
          {/* Badge */}
          <div className="absolute -top-2 -right-2 bg-purple-600 text-white text-xs px-2 py-0.5 rounded-full font-medium">
            Liquid AI
          </div>

          <div className="flex items-start gap-4">
            <div className="flex-shrink-0 w-12 h-12 bg-purple-100 rounded-lg flex items-center justify-center">
              <span className="text-2xl">🌊</span>
            </div>

            <div className="flex-1">
              <h3 className="font-semibold text-gray-900 mb-1">LFM2.5-Audio</h3>
              <p className="text-sm text-gray-600 mb-3">
                State-of-the-art 1.5B parameter model from Liquid AI with competitive transcription quality.
              </p>

              <div className="flex flex-wrap gap-3 text-xs text-gray-500 mb-4">
                <span className="flex items-center gap-1">
                  <HardDrive className="h-3 w-3" />
                  ~5GB total
                </span>
                <span className="flex items-center gap-1">
                  <Wifi className="h-3 w-3" />
                  ~3GB download
                </span>
                <span>7.53% WER</span>
                <span>GPU accelerated</span>
              </div>

              <Button
                onClick={startSetup}
                className="bg-purple-600 hover:bg-purple-700"
              >
                <Download className="mr-2 h-4 w-4" />
                Setup LFM2.5-Audio
              </Button>
            </div>
          </div>

          <Alert className="mt-4 border-amber-200 bg-amber-50">
            <AlertCircle className="h-4 w-4 text-amber-600" />
            <AlertTitle className="text-amber-800">Requirements</AlertTitle>
            <AlertDescription className="text-amber-700 text-sm">
              <ul className="list-disc list-inside space-y-1 mt-1">
                <li>Python 3.9+ installed on your system</li>
                <li>~5GB free disk space</li>
                <li>Internet connection for initial setup</li>
              </ul>
            </AlertDescription>
          </Alert>
        </motion.div>
      </div>
    );
  }

  // Setup in progress
  if (isSettingUp && setupProgress) {
    const stageInfo = getStageInfo(setupProgress.stage);
    const StageIcon = stageInfo.icon;

    return (
      <div className={`space-y-4 ${className}`}>
        <motion.div
          initial={{ opacity: 0, y: 5 }}
          animate={{ opacity: 1, y: 0 }}
          className="relative rounded-lg border-2 border-purple-300 bg-white p-5"
        >
          <div className="flex items-center gap-3 mb-4">
            <div className="flex-shrink-0 w-10 h-10 bg-purple-100 rounded-lg flex items-center justify-center">
              <Loader2 className="h-5 w-5 text-purple-600 animate-spin" />
            </div>
            <div>
              <h3 className="font-semibold text-gray-900">Setting up LFM2.5-Audio</h3>
              <p className="text-sm text-gray-500">Please wait, this takes 5-10 minutes...</p>
            </div>
          </div>

          {/* Progress bar */}
          <div className="mb-4">
            <div className="flex items-center justify-between mb-2">
              <div className="flex items-center gap-2">
                <StageIcon className={`h-4 w-4 ${stageInfo.color} ${setupProgress.stage !== 'complete' && setupProgress.stage !== 'failed' ? 'animate-pulse' : ''}`} />
                <span className="text-sm font-medium text-gray-700">{stageInfo.label}</span>
              </div>
              <span className="text-sm text-gray-500">{Math.round(setupProgress.progress * 100)}%</span>
            </div>
            <Progress value={setupProgress.progress * 100} className="h-2" />
          </div>

          {/* Current message */}
          <div className="bg-gray-50 rounded-md p-3">
            <p className="text-sm text-gray-700">{setupProgress.message}</p>
            {setupProgress.details && (
              <p className="text-xs text-gray-500 mt-1 font-mono truncate">{setupProgress.details}</p>
            )}
          </div>

          {/* Stage indicators */}
          <div className="mt-4 flex justify-between text-xs text-gray-400">
            {['Python', 'Environment', 'Dependencies', 'Model', 'Verify'].map((stage, i) => {
              const stages: SetupStage[] = ['checking_python', 'creating_venv', 'installing_dependencies', 'downloading_model', 'verifying'];
              const currentIndex = stages.indexOf(setupProgress.stage);
              const isComplete = i < currentIndex;
              const isCurrent = i === currentIndex;

              return (
                <div key={stage} className="flex flex-col items-center gap-1">
                  <div className={`w-6 h-6 rounded-full flex items-center justify-center text-xs font-medium
                    ${isComplete ? 'bg-green-500 text-white' : isCurrent ? 'bg-purple-500 text-white' : 'bg-gray-200 text-gray-500'}`}
                  >
                    {isComplete ? '✓' : i + 1}
                  </div>
                  <span className={isCurrent ? 'text-purple-600 font-medium' : ''}>{stage}</span>
                </div>
              );
            })}
          </div>
        </motion.div>
      </div>
    );
  }

  // Setup complete or model loaded - show model card
  return (
    <div className={`space-y-3 ${className}`}>
      <motion.div
        initial={{ opacity: 0, y: 5 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.2 }}
        className={`
          relative rounded-lg border-2 transition-all
          ${status === 'loaded'
            ? 'border-blue-500 bg-blue-50'
            : 'border-gray-200 bg-white hover:border-gray-300'
          }
        `}
      >
        {/* Liquid AI Badge */}
        <div className="absolute -top-2 -right-2 bg-purple-600 text-white text-xs px-2 py-0.5 rounded-full font-medium">
          Liquid AI
        </div>

        <div className="p-4">
          <div className="flex items-start justify-between mb-3">
            <div className="flex-1">
              {/* Model Name */}
              <div className="flex items-center gap-2 mb-1">
                <span className="text-2xl">🌊</span>
                <h3 className="font-semibold text-gray-900">LFM2.5-Audio</h3>
                {status === 'loaded' && (
                  <motion.span
                    initial={{ scale: 0 }}
                    animate={{ scale: 1 }}
                    className="bg-blue-600 text-white px-2 py-0.5 rounded-full text-xs font-medium flex items-center gap-1"
                  >
                    ✓ Active
                  </motion.span>
                )}
              </div>

              {/* Description */}
              <p className="text-sm text-gray-600 ml-9">
                1.5B parameter model • Competitive with Whisper • TTS capable
              </p>

              {/* Stats */}
              <div className="flex gap-4 mt-2 ml-9 text-xs text-gray-500">
                <span>~3GB model</span>
                <span>•</span>
                <span>7.53% WER</span>
                <span>•</span>
                <span>GPU accelerated</span>
              </div>
            </div>

            {/* Status/Action */}
            <div className="ml-4 flex items-center gap-2">
              {status === 'setup_complete' && (
                <Button
                  size="sm"
                  onClick={loadModel}
                  className="bg-purple-600 hover:bg-purple-700"
                >
                  <Play className="mr-2 h-4 w-4" />
                  Load Model
                </Button>
              )}

              {status === 'loading' && (
                <div className="flex items-center gap-2 text-purple-600">
                  <Loader2 className="h-4 w-4 animate-spin" />
                  <span className="text-sm font-medium">Loading...</span>
                </div>
              )}

              {status === 'loaded' && (
                <div className="flex items-center gap-2">
                  <div className="flex items-center gap-1.5 text-green-600">
                    <CheckCircle2 className="h-4 w-4" />
                    <span className="text-xs font-medium">Ready</span>
                  </div>
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={unloadModel}
                    className="text-gray-500 hover:text-red-600"
                  >
                    Unload
                  </Button>
                </div>
              )}

              {status === 'error' && (
                <Button
                  size="sm"
                  variant="destructive"
                  onClick={loadModel}
                >
                  Retry
                </Button>
              )}
            </div>
          </div>

          {/* Error message */}
          {error && (
            <div className="mt-3 p-2 bg-red-50 border border-red-200 rounded text-sm text-red-700">
              {error}
            </div>
          )}

          {/* Loading indicator */}
          {status === 'loading' && (
            <motion.div
              initial={{ opacity: 0, height: 0 }}
              animate={{ opacity: 1, height: 'auto' }}
              className="mt-3 pt-3 border-t border-gray-200"
            >
              <div className="flex items-center gap-2 mb-2">
                <Loader2 className="h-4 w-4 animate-spin text-purple-600" />
                <span className="text-sm text-gray-600">
                  Starting LFM engine...
                </span>
              </div>
            </motion.div>
          )}
        </div>
      </motion.div>

      {/* Helper text */}
      {status === 'loaded' && (
        <motion.div
          initial={{ opacity: 0, y: -5 }}
          animate={{ opacity: 1, y: 0 }}
          className="text-xs text-gray-500 text-center pt-2"
        >
          Using Liquid AI LFM2.5-Audio for transcription
        </motion.div>
      )}
    </div>
  );
}
