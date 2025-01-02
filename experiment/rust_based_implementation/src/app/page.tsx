'use client';

import { useState, useEffect, useContext } from 'react';
import { Transcript, Summary } from '@/types';
import { EditableTitle } from '@/components/EditableTitle';
import { TranscriptView } from '@/components/TranscriptView';
import { RecordingControls } from '@/components/RecordingControls';
import { AISummary } from '@/components/AISummary';
import { useSidebar } from '@/components/Sidebar/SidebarProvider';
import MainNav from '@/components/MainNav';
import { listen } from '@tauri-apps/api/event';

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
    introduction: {
        title: 'GitLab Release Kickoff Highlights',
        blocks: [
            { id: '1', type: 'bullet', content: 'GitLab 17.7 released, kicking off 17.8.', color: 'default' },
            { id: '2', type: 'bullet', content: 'Focus on FY25 investment themes: AI/ML efficiencies, use case adoption, platform differentiation, and SaaS capabilities.', color: 'gray' },
            { id: '3', type: 'bullet', content: 'Public Service Announcement: Roadmap details are subject to change.', color: 'default' }
        ]
    },
    devMonitorHighlights: {
        title: 'Dev and Monitor Updates',
        blocks: [
            { id: '4', type: 'bullet', content: 'Slash fix chat command added to VS Code for streamlined code fixes.', color: 'default' },
            { id: '5', type: 'bullet', content: 'Terminal output integration in Duo for improved debugging.', color: 'gray' },
            { id: '6', type: 'bullet', content: '20-40% latency improvement in code completion suggestions.', color: 'default' }
        ]
    },
    cicdUpdates: {
        title: 'CI/CD Enhancements',
        blocks: [
            { id: '7', type: 'bullet', content: 'Native integration with GitLab CMM for improved extensibility.', color: 'default' },
            { id: '8', type: 'bullet', content: 'Maven virtual registry enhancements for Java package interactions.', color: 'gray' },
            { id: '9', type: 'bullet', content: 'Focus on SaaS deployment improvements and secure workflows.', color: 'default' }
        ]
    },
    platformImprovements: {
        title: 'Platform Team Highlights',
        blocks: [
            { id: '10', type: 'bullet', content: 'Geo observability enhancements for system health monitoring.', color: 'default' },
            { id: '11', type: 'bullet', content: 'Backup and restore tool decoupling for improved load times.', color: 'gray' },
            { id: '12', type: 'bullet', content: 'User remappings via CSV for smoother project migrations.', color: 'default' }
        ]
    },
    securityEnhancements: {
        title: 'Security and Compliance Updates',
        blocks: [
            { id: '13', type: 'bullet', content: 'Custom permissions for managed security testing.', color: 'default' },
            { id: '14', type: 'bullet', content: 'Ability to override detected vulnerability severity.', color: 'gray' },
            { id: '15', type: 'bullet', content: 'Advanced SAST support for PHP.', color: 'default' }
        ]
    },
    dataScienceAdvances: {
        title: 'Data Science Enhancements',
        blocks: [
            { id: '16', type: 'bullet', content: 'AI-powered stage improvements for generative AI in Duo.', color: 'default' },
            { id: '17', type: 'bullet', content: 'Self-hosted AI Gateway configurations streamlined.', color: 'gray' },
            { id: '18', type: 'bullet', content: 'Enhanced Duo chat functionality and context injection.', color: 'default' }
        ]
    },
    closingRemarks: {
        title: 'Closing Notes',
        blocks: [
            { id: '19', type: 'bullet', content: 'Excitement for to-do list and PHP advancements.', color: 'default' },
            { id: '20', type: 'bullet', content: 'Recognition of Duo improvements enhancing productivity.', color: 'gray' },
            { id: '21', type: 'bullet', content: 'Encouragement for team efforts and milestone achievements.', color: 'default' }
        ]
    }
});


  const [isCollapsed, setIsCollapsed] = useState(false);

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
              return prev;
            }
            return [...prev, newTranscript];
          });
        });
      } catch (error) {
        console.error('Failed to setup transcript listener:', error);
      }
    };

    setupListener();

    return () => {
      if (unlistenFn) {
        unlistenFn();
      }
    };
  }, []);

  const handleRecordingStop = () => {
    setIsRecording(false);
    setIsSummaryLoading(true);
    // Show summary after 3 seconds
    setTimeout(() => {
      setShowSummary(true);
      setIsSummaryLoading(false);
      generateAISummary();
    }, 3000);
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

  const generateAISummary = () => {
    // Keep existing summary state as is
  };

  const handleSummaryChange = (newSummary: Summary) => {
    console.log('Summary changed:', newSummary);
    setAiSummary(newSummary);
  };

  const handleTitleChange = (newTitle: string) => {
    setMeetingTitle(newTitle);
    setCurrentMeeting({ id: 'intro-call', title: newTitle });
  };

  return (
    <div className="flex flex-col h-screen bg-gray-50">
      <MainNav />
      <div className="flex flex-1 overflow-hidden">
        {/* Left side - Transcript */}
        <div className="w-1/3 min-w-[300px] border-r border-gray-200 bg-white flex flex-col relative">
          {/* Title area */}
          <div className="p-4 border-b border-gray-200">
            <EditableTitle
              title={meetingTitle}
              isEditing={isEditingTitle}
              onEditStart={() => setIsEditingTitle(true)}
              onEditEnd={(newTitle) => {
                setMeetingTitle(newTitle);
                setIsEditingTitle(false);
              }}
            />
          </div>

          {/* Transcript content */}
          <div className="flex-1 overflow-y-auto pb-32">
            <TranscriptView transcripts={transcripts} />
          </div>

          {/* Recording controls with improved positioning */}
          <div className="absolute bottom-16 left-1/2 transform -translate-x-1/2 z-10">
            <div className="bg-white rounded-full shadow-lg">
              <RecordingControls
                isRecording={isRecording}
                onRecordingStop={handleRecordingStop}
                onRecordingStart={() => setIsRecording(true)}
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
              <AISummary summary={aiSummary} />
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
