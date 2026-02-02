export interface Message {
  id: string;
  content: string;
  timestamp: string;
}

export interface WordInfo {
  word: string;
  start: number;  // Start time in seconds
  end: number;    // End time in seconds
  probability: number;
}

export interface Transcript {
  id: string;
  text: string;
  timestamp: string; // Wall-clock time (e.g., "14:30:05")
  sequence_id?: number;
  chunk_start_time?: number; // Legacy field
  is_partial?: boolean;
  confidence?: number;
  // NEW: Recording-relative timestamps for playback sync
  audio_start_time?: number; // Seconds from recording start (e.g., 125.3)
  audio_end_time?: number;   // Seconds from recording start (e.g., 128.6)
  duration?: number;          // Segment duration in seconds (e.g., 3.3)
  // EagerMode: Two-tier transcription (confirmed vs hypothesis)
  confirmed_text?: string;    // Stable text that won't change
  hypothesis_text?: string;   // Text that may change with next update
  has_new_confirmed?: boolean; // Whether this update includes newly confirmed words
  words?: WordInfo[];         // Word-level timestamps
  // Nutshell-style streaming: phrase_id tracks streaming phrases
  // Same phrase_id = REPLACE existing entry (streaming update)
  // New phrase_id = ADD new entry (new phrase started after silence)
  phrase_id?: number;
}

export interface TranscriptUpdate {
  text: string;
  timestamp: string; // Wall-clock time for reference
  source: string;
  sequence_id: number;
  chunk_start_time: number; // Legacy field
  is_partial: boolean;
  confidence: number;
  // NEW: Recording-relative timestamps for playback sync
  audio_start_time: number; // Seconds from recording start
  audio_end_time: number;   // Seconds from recording start
  duration: number;          // Segment duration in seconds
  // EagerMode: Two-tier transcription (confirmed vs hypothesis)
  confirmed_text?: string;    // Stable text that won't change
  hypothesis_text?: string;   // Text that may change with next update
  has_new_confirmed?: boolean; // Whether this update includes newly confirmed words
  words?: WordInfo[];         // Word-level timestamps
  // Nutshell-style streaming: phrase_id tracks streaming phrases
  // Same phrase_id = REPLACE existing entry (streaming update)
  // New phrase_id = ADD new entry (new phrase started after silence)
  phrase_id?: number;
}

export interface Block {
  id: string;
  type: string;
  content: string;
  color: string;
}

export interface Section {
  title: string;
  blocks: Block[];
}

export interface Summary {
  [key: string]: Section;
}

export interface ApiResponse {
  message: string;
  num_chunks: number;
  data: any[];
}

export interface SummaryResponse {
  status: string;
  summary: Summary;
  raw_summary?: string;
  usage?: {
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
  };
}

// BlockNote-specific types
export type SummaryFormat = 'legacy' | 'markdown' | 'blocknote';

export interface BlockNoteBlock {
  id: string;
  type: string;
  props?: Record<string, any>;
  content?: any[];
  children?: BlockNoteBlock[];
}

export interface SummaryDataResponse {
  markdown?: string;
  summary_json?: BlockNoteBlock[];
  // Legacy format fields
  MeetingName?: string;
  _section_order?: string[];
  [key: string]: any; // For legacy section data
}

// Pagination types for optimized transcript loading
export interface MeetingMetadata {
  id: string;
  title: string;
  created_at: string;
  updated_at: string;
  folder_path?: string;
}

export interface PaginatedTranscriptsResponse {
  transcripts: Transcript[];
  total_count: number;
  has_more: boolean;
}

// Transcript segment data for virtualized display
export interface TranscriptSegmentData {
  id: string;
  timestamp: number; // audio_start_time in seconds
  endTime?: number; // audio_end_time in seconds
  text: string;
  confidence?: number;
  // EagerMode: Two-tier transcription
  confirmed_text?: string;    // Stable text that won't change
  hypothesis_text?: string;   // Text that may change (display in gray/italic)
  has_new_confirmed?: boolean;
  // Nutshell-style streaming: phrase_id for in-place updates
  phrase_id?: number;
}
