import { test, expect } from '@playwright/test';
import { bootstrap, speakerCalls } from './_speaker-helpers';

// Smoke spec for recognized-name-badge-revert: the undo icon on a RECOGNIZED
// speaker badge (diarization matched a stamped speaker, previous_label NULL)
// must drive the full loop — revert_speaker_label dispatch carrying the
// recognized name, refetch, badge re-rendering the recovered cluster label
// ("Speaker X"). Regression for the silent no-op: the icon used to render for
// recognized names while the revert matched 0 rows, so nothing visibly
// happened. Backend recovery semantics are pinned by the Rust tests
// (recognized_name_revert_restores_cluster_label_and_unlinks et al.); this
// spec pins the UI wiring.

interface SmokeDispatcher {
  register(cmd: string, handler: (args: Record<string, unknown>) => unknown): void;
}

const RECOGNIZED_ROW = {
  id: 't1',
  text: 'Cynthia lays out the migration plan.',
  timestamp: '00:00:01',
  audio_start_time: 0,
  speaker: 'Cynthia Wu',
};

test.describe('recognized-name badge revert smoke', () => {
  test.beforeEach(() => {
    // Cold first-compile of /meeting-details is slow; match the sibling specs' budget.
    test.setTimeout(120_000);
  });

  test('1.1 — undo on a recognized name dispatches revert_speaker_label and the badge reverts to the cluster label', async ({ page }) => {
    await bootstrap(page, [RECOGNIZED_ROW]);

    // The badge's accessible name is the content concatenation (it embeds the
    // undo button's aria-label), so locate it via its inner text span — exact.
    const badge = page.getByText('Cynthia Wu', { exact: true });
    await expect(badge).toBeVisible({ timeout: 20_000 });

    // The mock's transcript fixture is static; mutate it to the post-revert
    // state when the command fires so the refetch (onSpeakersChanged) renders
    // what the real backend persists: the recovered cluster label. The
    // handler keeps recording into __smokeSpeakerCalls like the stock one.
    await page.evaluate(() => {
      const w = window as unknown as {
        __tauriMockDispatcher: SmokeDispatcher;
        __smokeSpeakerCalls: Array<Record<string, unknown>>;
        __smokeTranscripts: Array<{ speaker?: string }>;
      };
      w.__tauriMockDispatcher.register('revert_speaker_label', function (args) {
        const a = args as { meetingId: string; speakerLabel: string };
        w.__smokeSpeakerCalls.push({
          cmd: 'revert_speaker_label',
          meetingId: a.meetingId,
          speakerLabel: a.speakerLabel,
        });
        w.__smokeTranscripts = w.__smokeTranscripts.map((t) =>
          t.speaker === 'Cynthia Wu' ? { ...t, speaker: 'Speaker 1' } : t,
        );
        return 1;
      });
    });

    // The undo icon sits in a zero-width span until the badge is hovered.
    await badge.hover();
    const undo = page.getByRole('button', { name: 'Revert Cynthia Wu to original label', exact: true });
    await expect(undo).toBeVisible({ timeout: 5_000 });
    await undo.click();

    // The dispatch carries the recognized name, not the cluster label.
    await expect.poll(async () => {
      const calls = await speakerCalls(page);
      return calls.find((c) => c.cmd === 'revert_speaker_label') ?? null;
    }, { timeout: 10_000 }).toEqual({
      cmd: 'revert_speaker_label',
      meetingId: 'meet-summary-001',
      speakerLabel: 'Cynthia Wu',
    });

    // stopPropagation: undo must not also open the inline rename editor.
    await expect(page.getByPlaceholder('Enter speaker name...')).toHaveCount(0);

    // Refetch renders the recovered cluster label; the recognized badge is gone.
    await expect(page.getByRole('button', { name: 'Speaker 1', exact: true })).toBeVisible({ timeout: 15_000 });
    await expect(page.getByText('Cynthia Wu', { exact: true })).toHaveCount(0);
  });

  test('1.2 — undo is not offered on auto-generated labels', async ({ page }) => {
    await bootstrap(page, [
      { id: 't2', text: 'Speaker zero talking.', timestamp: '00:00:02', audio_start_time: 0, speaker: 'Speaker 0' },
    ]);

    const badge = page.getByRole('button', { name: 'Speaker 0', exact: true });
    await expect(badge).toBeVisible({ timeout: 20_000 });
    await badge.hover();

    await expect(page.getByRole('button', { name: /Revert Speaker 0/ })).toHaveCount(0);
  });
});
