import { describe, it, expect } from 'vitest';
import { computeDisplayText } from '@/components/VirtualizedTranscriptView';

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
