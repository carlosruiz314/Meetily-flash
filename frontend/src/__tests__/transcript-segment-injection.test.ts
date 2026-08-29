import { describe, it, expect } from 'vitest';
import {
    computeDisplayText,
    isContinuation,
    endsSentence,
    startsMidSentence,
} from '@/components/VirtualizedTranscriptView';

// §4 adversarial category: prompt injection / XSS via transcript text.
//
// TranscriptSegment renders `text` inside <p>{displayText}</p>. React escapes
// text content by default, so adversarial HTML in the transcript is inert as
// long as (a) the processing layer passes text through without transforming it
// and (b) no dangerouslySetInnerHTML appears in the render path (guarded by
// the separate dangerouslySetInnerHTML lint/test in task 8.4).
//
// computeDisplayText must also be content-preserving: the view renders the
// transcript VERBATIM. It used to silently strip "filler words" (oh, uh, um…)
// which deleted spoken words from the user's record — a view must never
// rewrite the transcript.
describe('computeDisplayText — adversarial payloads pass through unchanged (§4)', () => {
  const adversarial: Array<{ label: string; payload: string }> = [
    { label: '<script> tag', payload: '<script>alert(1)</script>' },
    { label: '<img onerror>', payload: '<img src=x onerror="alert(1)">' },
    { label: '<svg onload>', payload: '<svg onload="alert(1)">' },
    { label: '<iframe>', payload: '<iframe src="javascript:alert(1)"></iframe>' },
    { label: 'javascript: URL', payload: 'javascript:alert(1)' },
    { label: 'template injection ${...}', payload: '${7*7}' },
    { label: 'mixed-case <SCRIPT>', payload: '<SCRIPT>alert(1)</SCRIPT>' },
    { label: 'SQL-style injection in prose', payload: "'; DROP TABLE meetings; --" },
  ];

  for (const { label, payload } of adversarial) {
    it(`does not sanitize or restructure ${label} — passes through for React to escape`, () => {
      expect(computeDisplayText(payload)).toBe(payload.replace(/\s+/g, ' ').trim());
    });
  }

  it('collapses to [Silence] for empty/whitespace-only text', () => {
    expect(computeDisplayText('')).toBe('[Silence]');
    expect(computeDisplayText('   ')).toBe('[Silence]');
  });

  it('renders spoken words verbatim — filler words are content, not noise', () => {
    expect(computeDisplayText('uh hello world um')).toBe('uh hello world um');
    expect(computeDisplayText("That's right. Oh, man")).toBe("That's right. Oh, man");
  });

  it('never drops words around adversarial payload', () => {
    expect(computeDisplayText('uh <script>alert(1)</script> um')).toBe(
      'uh <script>alert(1)</script> um'
    );
  });
});

// Interruption rendering: when people finish each other's sentences, one
// sentence spans two speaker rows. The view marks the continuation instead of
// letting it read as a random break.
describe('isContinuation — marks interrupted sentences across speakers', () => {
  it('detects an interrupted previous turn followed by a lowercase resume', () => {
    expect(isContinuation("I don't like you've aged like", 'five years. Yeah.')).toBe(true);
  });

  it('does not mark a completed previous turn', () => {
    expect(isContinuation('That is a lesson I have learned.', 'Yeah. So for search,')).toBe(false);
  });

  it('does not mark a capitalized resume even if previous turn is open', () => {
    expect(isContinuation('and then he said', 'Wait, no way.')).toBe(false);
  });

  it('handles trailing quotes and brackets on the sentence end', () => {
    expect(endsSentence('He said "stop."')).toBe(true);
    expect(endsSentence('He said "stop')).toBe(false);
  });

  it('lowercase detection ignores leading punctuation and whitespace', () => {
    expect(startsMidSentence('  ... to, let\'s')).toBe(true);
    expect(startsMidSentence('Okay. I have updates')).toBe(false);
    expect(startsMidSentence('')).toBe(false);
  });
});
