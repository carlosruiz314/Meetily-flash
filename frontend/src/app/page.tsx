'use client';

import { useState, useEffect, useContext, useCallback } from 'react';
import { Transcript, Summary, SummaryResponse } from '@/types';
import { EditableTitle } from '@/components/EditableTitle';
import { TranscriptView } from '@/components/TranscriptView';
import { RecordingControls } from '@/components/RecordingControls';
import { AISummary } from '@/components/AISummary';
import { useSidebar } from '@/components/Sidebar/SidebarProvider';
import MainNav from '@/components/MainNav';
import { listen } from '@tauri-apps/api/event';
import { writeTextFile } from '@tauri-apps/plugin-fs';
import { downloadDir } from '@tauri-apps/api/path';

interface TranscriptUpdate {
  text: string;
  timestamp: string;
  source: string;
}

export default function Home() {
  const [isRecording, setIsRecording] = useState(false);
  const [transcripts, setTranscripts] = useState<Transcript[]>([]);
  const [showSummary, setShowSummary] = useState(false);
  const [isSummaryLoading, setIsSummaryLoading] = useState(false);
  const [barHeights, setBarHeights] = useState(['58%', '76%', '58%']);
  const [meetingTitle, setMeetingTitle] = useState('New Call');
  const [isEditingTitle, setIsEditingTitle] = useState(false);
  const [aiSummary, setAiSummary] = useState<Summary>({
    podcastOverview: {
        title: 'Podcast Discussion Highlights',
        blocks: [
            { id: '1', type: 'bullet', content: 'Exploration of venture capital (VC) sourcing strategies and challenges.', color: 'default' },
            { id: '2', type: 'bullet', content: 'Insights on investment risk and the role of accredited investors.', color: 'gray' },
            { id: '3', type: 'bullet', content: 'Discussion on the evolution and specialization of VC practices.', color: 'default' }
        ]
    },
    sourcingInsights: {
        title: 'How VC Firms Source Investments',
        blocks: [
            { id: '4', type: 'bullet', content: 'Companies often found through public announcements, events, and conferences.', color: 'default' },
            { id: '5', type: 'bullet', content: 'Venture arms play a key role in identifying startups seeking funding.', color: 'gray' },
            { id: '6', type: 'bullet', content: 'Networking at industry forums and academic gatherings is crucial.', color: 'default' }
        ]
    },
    investmentChallenges: {
        title: 'Challenges in Venture Investments',
        blocks: [
            { id: '7', type: 'bullet', content: 'High-risk business with only 1 out of 100 investments typically succeeding.', color: 'default' },
            { id: '8', type: 'bullet', content: 'Success rates vary significantly between established and new VC firms.', color: 'gray' },
            { id: '9', type: 'bullet', content: 'Equity investments can result in total loss if the company fails.', color: 'default' }
        ]
    },
    perspectivesOnVC: {
        title: 'Perspectives on VC Practices',
        blocks: [
            { id: '10', type: 'bullet', content: 'Comparison of VCs to the evolution of surgeons highlights growth potential.', color: 'default' },
            { id: '11', type: 'bullet', content: 'Role of trust in the people behind startups as a key investment factor.', color: 'gray' },
            { id: '12', type: 'bullet', content: 'Challenges in maintaining value and protecting investments discussed.', color: 'default' }
        ]
    },
    personalReflections: {
        title: 'Personal Reflections and Insights',
        blocks: [
            { id: '13', type: 'bullet', content: 'Importance of the ability to “get things done” highlighted.', color: 'default' },
            { id: '14', type: 'bullet', content: 'Inspirations from Steve Jobs and his approach to innovation.', color: 'gray' },
            { id: '15', type: 'bullet', content: 'Reflections on the significance of academic and early professional experiences.', color: 'default' }
        ]
    },
    closingRemarks: {
        title: 'Closing Remarks',
        blocks: [
            { id: '16', type: 'bullet', content: 'Appreciation for the conversation and insights shared.', color: 'default' },
            { id: '17', type: 'bullet', content: 'Emphasis on the lasting impact of early professional relationships.', color: 'gray' },
            { id: '18', type: 'bullet', content: 'Host expresses gratitude for the guest’s participation.', color: 'default' }
        ]
    }
  });
  const [summaryResponse, setSummaryResponse] = useState<SummaryResponse | null>(null);

  const [isCollapsed, setIsCollapsed] = useState(false);

  const [summaryStatus, setSummaryStatus] = useState<'idle' | 'processing' | 'summarizing' | 'completed' | 'error'>('idle');
  const [summaryError, setSummaryError] = useState<string | null>(null);

  const { setCurrentMeeting } = useSidebar();

  useEffect(() => {
    setCurrentMeeting({ id: 'intro-call', title: meetingTitle });
  }, [meetingTitle, setCurrentMeeting]);

  useEffect(() => {
    if (isRecording) {
      const interval = setInterval(() => {
        setBarHeights(prev => {
          const newHeights = [...prev];
          newHeights[0] = Math.random() * 20 + 10 + 'px';
          newHeights[1] = Math.random() * 20 + 10 + 'px';
          newHeights[2] = Math.random() * 20 + 10 + 'px';
          return newHeights;
        });
      }, 300);

      return () => clearInterval(interval);
    }
  }, [isRecording]);

  useEffect(() => {
    let unlistenFn: (() => void) | undefined;
    let transcriptCounter = 0;  // Counter for unique IDs

    const setupListener = async () => {
      try {
        console.log('Setting up transcript listener...');
        unlistenFn = await listen<TranscriptUpdate>('transcript-update', (event) => {
          console.log('Received transcript update:', event.payload);
          const newTranscript = {
            id: `${Date.now()}-${transcriptCounter++}`,  // Combine timestamp with counter for uniqueness
            text: event.payload.text,
            timestamp: event.payload.timestamp,
          };
          setTranscripts(prev => {
            // Check if this transcript already exists
            const exists = prev.some(
              t => t.text === event.payload.text && t.timestamp === event.payload.timestamp
            );
            if (exists) {
              console.log('Duplicate transcript, skipping:', newTranscript);
              return prev;
            }
            console.log('Adding new transcript:', newTranscript);
            return [...prev, newTranscript];
          });
        });
        console.log('Transcript listener setup complete');
      } catch (error) {
        console.error('Failed to setup transcript listener:', error);
        alert('Failed to setup transcript listener. Check console for details.');
      }
    };

    setupListener();
    console.log('Started listener setup');

    return () => {
      console.log('Cleaning up transcript listener...');
      if (unlistenFn) {
        unlistenFn();
        console.log('Transcript listener cleaned up');
      }
    };
  }, []);

  const handleRecordingStart = async () => {
    try {
      console.log('Starting recording...');
      const { invoke } = await import('@tauri-apps/api/core');
      
      // First check if we're already recording
      const isCurrentlyRecording = await invoke('is_recording');
      if (isCurrentlyRecording) {
        console.log('Already recording, stopping first...');
        await handleRecordingStop();
      }

      // Start new recording
      await invoke('start_recording');
      console.log('Recording started successfully');
      setIsRecording(true);
      setTranscripts([]); // Clear previous transcripts when starting new recording
    } catch (error) {
      console.error('Failed to start recording:', error);
      alert('Failed to start recording. Check console for details.');
      setIsRecording(false); // Reset state on error
    }
  };

  const handleRecordingStop = async () => {
    try {
      console.log('Stopping recording...');
      const { invoke } = await import('@tauri-apps/api/core');
      const { appDataDir } = await import('@tauri-apps/api/path');
      
      const dataDir = await appDataDir();
      const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
      const transcriptPath = `${dataDir}transcript-${timestamp}.txt`;
      const audioPath = `${dataDir}recording-${timestamp}.wav`;

      // Stop recording and save audio
      await invoke('stop_recording', { 
        args: { save_path: audioPath }
      });
      console.log('Recording stopped successfully');

      // Format and save transcript
      const formattedTranscript = transcripts
        .sort((a, b) => new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime())
        .map(t => `[${t.timestamp}] ${t.text}`)
        .join('\n\n');

      const documentContent = `Meeting Title: ${meetingTitle}\nDate: ${new Date().toLocaleString()}\n\nTranscript:\n${formattedTranscript}`;

      await invoke('save_transcript', { 
        filePath: transcriptPath,
        content: documentContent
      });
      console.log('Transcript saved to:', transcriptPath);

      setIsRecording(false);
      setIsSummaryLoading(true);

      // Show summary after saving transcript
      setTimeout(() => {
        setShowSummary(true);
        setIsSummaryLoading(false);
        generateAISummary();
      }, 3000);
    } catch (error) {
      console.error('Failed to stop recording:', error);
      if (error instanceof Error) {
        console.error('Error details:', {
          message: error.message,
          name: error.name,
          stack: error.stack,
        });
      }
      alert('Failed to stop recording. Check console for details.');
      setIsRecording(false); // Reset state on error
    }
  };

  const handleTranscriptUpdate = (update: any) => {
    console.log('Handling transcript update:', update);
    const newTranscript = {
      id: Date.now().toString(),
      text: update.text,
      timestamp: update.timestamp,
    };
    setTranscripts(prev => {
      // Check if this transcript already exists
      const exists = prev.some(
        t => t.text === update.text && t.timestamp === update.timestamp
      );
      if (exists) {
        return prev;
      }
      return [...prev, newTranscript];
    });
  };

  const generateAISummary = useCallback(async () => {
    setSummaryStatus('processing');
    setSummaryError(null);

    try {
      const fullTranscript = transcripts.map(t => t.text).join('\n');
      if (!fullTranscript.trim()) {
        throw new Error('No transcript text available. Please add some text first.');
      }
      
      console.log('Generating summary for transcript length:', fullTranscript.length);
      
      // Process transcript and get summary in one call
      console.log('Processing transcript and generating summary...');
      const response = await fetch('http://localhost:5167/process-transcript', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          text: fullTranscript,
          chunk_size: 5000,
          overlap: 1000
        })
      });

      if (!response.ok) {
        const errorData = await response.json().catch(() => ({ detail: 'Failed to process transcript' }));
        console.error('Process transcript failed:', errorData);
        throw new Error(errorData.detail || 'Failed to process transcript');
      }

      const result = await response.json();
      console.log('Process transcript response:', result);

      // Validate the response structure
      if (!result.data) {
        throw new Error('Invalid response format from server');
      }

      // Format the summary data with consistent styling
      const formattedSummary = Object.entries(result.data).reduce((acc: Summary, [key, section]: [string, any]) => {
        acc[key] = {
          title: section.title,
          blocks: section.blocks.map((block: any) => ({
            ...block,
            type: 'bullet',
            color: 'default',
            content: block.content.trim() // Remove trailing newlines
          }))
        };
        return acc;
      }, {} as Summary);

      setAiSummary(formattedSummary);
      setSummaryStatus('completed');
      
    } catch (error) {
      console.error('Failed to generate summary:', error);
      setSummaryError(error instanceof Error ? error.message : 'An unexpected error occurred');
      setSummaryStatus('error');
      setAiSummary(null);
    }
  }, [transcripts]);

  const handleSummary = useCallback((summary: any) => {
    setAiSummary(summary);
  }, []);

  const handleSummaryChange = (newSummary: Summary) => {
    console.log('Summary changed:', newSummary);
    setAiSummary(newSummary);
  };

  const handleTitleChange = (newTitle: string) => {
    setMeetingTitle(newTitle);
    setCurrentMeeting({ id: 'intro-call', title: newTitle });
  };

  const getSummaryStatusMessage = () => {
    switch (summaryStatus) {
      case 'processing':
        return 'Processing transcript...';
      case 'summarizing':
        return 'Generating AI summary...';
      case 'completed':
        return 'Summary generated successfully!';
      case 'error':
        return summaryError || 'An error occurred while generating the summary';
      default:
        return '';
    }
  };

  const handleDownloadTranscript = async () => {
    try {
      // Create transcript object with metadata
      const transcriptData = {
        title: meetingTitle,
        timestamp: new Date().toISOString(),
        transcripts: transcripts
      };

      // Generate filename
      const sanitizedTitle = meetingTitle.replace(/[^a-zA-Z0-9]/g, '_');
      const filename = `${sanitizedTitle}_transcript.json`;
      
      // Get download directory path
      const downloadPath = await downloadDir();
      
      // Write file to downloads directory
      await writeTextFile(`${downloadPath}/${filename}`, JSON.stringify(transcriptData, null, 2));

      console.log('Transcript saved successfully to:', `${downloadPath}/${filename}`);
      alert('Transcript downloaded successfully!');
    } catch (error) {
      console.error('Failed to save transcript:', error);
      alert('Failed to save transcript. Please try again.');
    }
  };

  const handleUploadTranscript = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    try {
      const text = await file.text();
      const data = JSON.parse(text);
      
      // Validate the uploaded file structure
      if (!data.transcripts || !Array.isArray(data.transcripts)) {
        throw new Error('Invalid transcript file format');
      }

      // Update state with uploaded data
      setMeetingTitle(data.title || 'Uploaded Transcript');
      setTranscripts(data.transcripts);
      
      // Generate summary for the uploaded transcript
      handleSummary(data.transcripts);
    } catch (error) {
      console.error('Error uploading transcript:', error);
      alert('Failed to upload transcript. Please make sure the file format is correct.');
    }
  };

  return (
    <div className="flex flex-col h-screen bg-gray-50">
      <MainNav title={meetingTitle} />
      <div className="flex flex-1 overflow-hidden">
        {/* Left side - Transcript */}
        <div className="w-1/3 min-w-[300px] border-r border-gray-200 bg-white flex flex-col relative">
          {/* Title area */}
          <div className="p-4 border-b border-gray-200">
            <div className="flex items-center justify-between">
              <EditableTitle
                title={meetingTitle}
                isEditing={isEditingTitle}
                onStartEditing={() => setIsEditingTitle(true)}
                onFinishEditing={() => setIsEditingTitle(false)}
                onChange={handleTitleChange}
              />
              <div className="flex items-center">
                <button
                  onClick={handleDownloadTranscript}
                  className="ml-2 p-2 text-gray-600 hover:text-gray-800 rounded-full hover:bg-gray-100"
                  title="Download Transcript"
                >
                  <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" strokeWidth={1.5} stroke="currentColor" className="w-5 h-5">
                    <path strokeLinecap="round" strokeLinejoin="round" d="M3 16.5v2.25A2.25 2.25 0 005.25 21h13.5A2.25 2.25 0 0021 18.75V16.5M16.5 12L12 16.5m0 0L7.5 12m4.5 4.5V3" />
                  </svg>
                </button>
                <label className="ml-2 p-2 text-gray-600 hover:text-gray-800 rounded-full hover:bg-gray-100 cursor-pointer" title="Upload Transcript">
                  <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" strokeWidth={1.5} stroke="currentColor" className="w-5 h-5">
                    <path strokeLinecap="round" strokeLinejoin="round" d="M3 16.5v2.25A2.25 2.25 0 005.25 21h13.5A2.25 2.25 0 0021 18.75V16.5M16.5 12L12 7.5m0 0L7.5 12m4.5-4.5v12" />
                  </svg>
                  <input
                    type="file"
                    accept=".json"
                    onChange={handleUploadTranscript}
                    className="hidden"
                  />
                </label>
              </div>
            </div>
          </div>

          {/* Transcript content */}
          <div className="flex-1 overflow-y-auto pb-32">
            <TranscriptView transcripts={transcripts} />
          </div>

          {/* Recording controls */}
          <div className="absolute bottom-16 left-1/2 transform -translate-x-1/2 z-10">
            <div className="bg-white rounded-full shadow-lg">
              <RecordingControls
                isRecording={isRecording}
                onRecordingStop={handleRecordingStop}
                onRecordingStart={handleRecordingStart}
                onTranscriptReceived={handleSummary}
                barHeights={barHeights}
              />
            </div>
          </div>
        </div>

        {/* Right side - AI Summary */}
        <div className="flex-1 overflow-y-auto bg-white">
          {isSummaryLoading ? (
            <div className="flex items-center justify-center h-full">
              <div className="text-center">
                <div className="inline-block animate-spin rounded-full h-12 w-12 border-t-2 border-b-2 border-blue-500 mb-4"></div>
                <p className="text-gray-600">Generating AI Summary...</p>
              </div>
            </div>
          ) : showSummary && (
            <div className="max-w-4xl mx-auto p-6">
              {summaryResponse && (
                <div className="fixed bottom-0 left-0 right-0 bg-white shadow-lg p-4 max-h-1/3 overflow-y-auto">
                  <h3 className="text-lg font-semibold mb-2">Meeting Summary</h3>
                  <div className="grid grid-cols-2 gap-4">
                    <div>
                      <h4 className="font-medium mb-1">Key Points</h4>
                      <ul className="list-disc pl-4">
                        {summaryResponse.summary.key_points.map((point, i) => (
                          <li key={i} className="text-sm">{point}</li>
                        ))}
                      </ul>
                    </div>
                    <div>
                      <h4 className="font-medium mb-1">Action Items</h4>
                      <ul className="list-disc pl-4">
                        {summaryResponse.summary.action_items.map((item, i) => (
                          <li key={i} className="text-sm">{item}</li>
                        ))}
                      </ul>
                    </div>
                    <div>
                      <h4 className="font-medium mb-1">Decisions</h4>
                      <ul className="list-disc pl-4">
                        {summaryResponse.summary.decisions.map((decision, i) => (
                          <li key={i} className="text-sm">{decision}</li>
                        ))}
                      </ul>
                    </div>
                    <div>
                      <h4 className="font-medium mb-1">Main Topics</h4>
                      <ul className="list-disc pl-4">
                        {summaryResponse.summary.main_topics.map((topic, i) => (
                          <li key={i} className="text-sm">{topic}</li>
                        ))}
                      </ul>
                    </div>
                  </div>
                  {summaryResponse.raw_summary && (
                    <div className="mt-4">
                      <h4 className="font-medium mb-1">Full Summary</h4>
                      <p className="text-sm whitespace-pre-wrap">{summaryResponse.raw_summary}</p>
                    </div>
                  )}
                </div>
              )}
              <div className="flex-1 overflow-y-auto p-4">
                <AISummary 
                  summary={aiSummary} 
                  status={summaryStatus} 
                  error={summaryError}
                  onSummaryChange={(newSummary) => setAiSummary(newSummary)}
                />
              </div>
              {summaryStatus !== 'idle' && (
                <div className={`mt-4 p-4 rounded-lg ${
                  summaryStatus === 'error' ? 'bg-red-100 text-red-700' :
                  summaryStatus === 'completed' ? 'bg-green-100 text-green-700' :
                  'bg-blue-100 text-blue-700'
                }`}>
                  <p className="text-sm font-medium">{getSummaryStatusMessage()}</p>
                </div>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
