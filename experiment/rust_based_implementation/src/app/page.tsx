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

    const setupListener = async () => {
      try {
        unlistenFn = await listen<TranscriptUpdate>('transcript-update', (event) => {
          console.log('Received transcript update:', event.payload);
          const newTranscript = {
            id: Date.now().toString(),
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
          <div className="p-4 border-b border-gray-200">
            <EditableTitle
              title={meetingTitle}
              isEditing={isEditingTitle}
              onEdit={setMeetingTitle}
              onEditingChange={setIsEditingTitle}
            />
          </div>
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
              <AISummary summary={aiSummary} onSummaryChange={setAiSummary} />
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
