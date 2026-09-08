import { test, expect } from '@playwright/test';
import { TAURI_MOCK_INIT_SCRIPT } from '../mocks/init-script';
import { SMOKE_DEFAULTS_INIT_SCRIPT, SMOKE_MEETING_ID, SMOKE_MEETING_TITLE } from './_defaults';

// Regression for the stale "1 queued" indicator: deleting a meeting that had a
// queued transcription job removed only the SQLite row — the in-memory
// TranscriptionQueue kept the job and GlobalQueueIndicator kept counting it.
//
// What this spec proves (real webview, real React state, real event bus):
//   record → stop (enqueues via the isCallApi path) → "1 queued" banner mounts
//   → delete the meeting from its sidebar row → banner unmounts.
//
// What it deliberately cannot prove: that the REAL Rust api_delete_meeting
// cancels the queue job. The browser harness routes every invoke through the
// mock dispatcher, so the override below MODELS the fixed backend contract
// (delete row → cancel job → emit fresh transcription-queue-changed snapshot).
// If the frontend stops reacting to that snapshot — or the delete flow stops
// triggering it — this spec fails. The Rust half is pinned by review + the
// manual Tauri smoke test (see the fix commit).

const QUEUE_DELETE_OVERRIDES_INIT_SCRIPT = `
(function () {
  'use strict';
  var d = window.__tauriMockDispatcher;
  if (!d) return;
  var bus = window.__tauriMockEventBus;

  window.__smokeQueue = [];

  function snapshot() {
    return { jobs: window.__smokeQueue.slice(), manual_pause_all: false };
  }

  // Contract of the real enqueue_transcription_job (lib.rs): push the job,
  // then emit the updated snapshot so every listener updates immediately.
  // meetingId is read strictly — a frontend regression that drops/renames the
  // arg must fail here, not be masked by a fallback.
  d.register('enqueue_transcription_job', function (args) {
    window.__smokeQueue.push({
      meeting_id: args.meetingId,
      audio_path: args.audioPath,
      status: 'Pending',
      phase: 'Transcribing',
      pause_reason: null,
    });
    if (bus) bus.emit('transcription-queue-changed', snapshot());
    return null;
  });

  d.register('get_queue_state', function () {
    return snapshot();
  });

  // Contract of the FIXED api_delete_meeting (api.rs): on success, cancel the
  // meeting's queue job and emit the fresh snapshot in the same invoke.
  d.register('api_delete_meeting', function (args) {
    var id = args && args.meetingId;
    window.__smokeMeetings = (window.__smokeMeetings || []).filter(function (m) {
      return m.id !== id;
    });
    window.__smokeQueue = window.__smokeQueue.filter(function (j) {
      return j.meeting_id !== id;
    });
    if (bus) bus.emit('transcription-queue-changed', snapshot());
    return { status: 'success', message: 'Meeting deleted successfully' };
  });

  // Sidebar QueueStatusBadge renders a per-meeting cancel button; keep the
  // command in the mock contract so an accidental click resolves cleanly.
  d.register('cancel_queued_job', function (args) {
    var id = args && args.meetingId;
    window.__smokeQueue = window.__smokeQueue.filter(function (j) {
      return j.meeting_id !== id;
    });
    if (bus) bus.emit('transcription-queue-changed', snapshot());
    return null;
  });
})();
`;

async function callLogIncludes(page: import('@playwright/test').Page, cmd: string): Promise<boolean> {
  return page.evaluate(
    (c) => (window as unknown as { __tauriMockDispatcher: { callLog: () => string[] } })
      .__tauriMockDispatcher.callLog().includes(c),
    cmd,
  );
}

test.describe('queue-delete-clears-banner smoke', () => {
  test('record → enqueue shows "1 queued" → deleting the meeting clears it', async ({ page }) => {
    const pageErrors: string[] = [];
    page.on('pageerror', (e) => pageErrors.push(e.message));
    page.on('dialog', (d) => d.dismiss());

    await page.addInitScript(TAURI_MOCK_INIT_SCRIPT);
    await page.addInitScript(SMOKE_DEFAULTS_INIT_SCRIPT);
    await page.addInitScript(QUEUE_DELETE_OVERRIDES_INIT_SCRIPT);
    await page.goto('/');
    await page.waitForFunction(
      () => (window as unknown as { __tauriCoreMockActive?: boolean }).__tauriCoreMockActive === true,
      { timeout: 15_000 },
    );

    // ── Record a (fake) meeting ────────────────────────────────────────────
    const sidebarMic = page.locator('button.bg-red-500').filter({
      has: page.locator('svg.lucide-mic'),
    });
    await expect(sidebarMic).toBeVisible({ timeout: 15_000 });
    await sidebarMic.click();

    const stopButton = page.locator('button:not([disabled])').filter({
      has: page.locator('svg.lucide-square'),
    });
    await expect(stopButton).toBeVisible({ timeout: 15_000 });
    await stopButton.click();

    // The stop path (isCallApi=true) must have enqueued the transcription job.
    await expect.poll(() => callLogIncludes(page, 'enqueue_transcription_job'), {
      timeout: 15_000,
    }).toBe(true);

    // ── Expand the sidebar rail ────────────────────────────────────────────
    // The rail starts collapsed (SidebarProvider useState(true)); the banner
    // and meeting rows only render in the expanded rail. The floating
    // chevron button is a pure toggleCollapse — unlike the NotebookPen
    // shortcut it does not also toggle the meetings folder.
    // lucide 0.469 renders ChevronRightCircle (an alias) with the canonical
    // class name lucide-circle-chevron-right.
    const expandRail = page.locator('button').filter({
      has: page.locator('svg.lucide-circle-chevron-right'),
    });
    await expect(expandRail).toBeVisible({ timeout: 15_000 });
    await expandRail.click();

    // ── The banner counts the queued job ───────────────────────────────────
    // Mock job status is Pending → indicator label is exactly "1 queued".
    const banner = page.getByText('1 queued', { exact: true });
    await expect(banner).toBeVisible({ timeout: 15_000 });

    // ── Delete the meeting immediately, via its sidebar row ────────────────
    const meetingRow = page.locator('span.flex-1', { hasText: SMOKE_MEETING_TITLE }).first();
    await expect(meetingRow).toBeVisible({ timeout: 15_000 });

    // The row's hover actions (opacity-0 until hover) include the delete button.
    const deleteButton = page.getByRole('button', { name: 'Delete meeting' }).first();
    await deleteButton.click();

    const confirmButton = page.getByRole('button', { name: 'Delete', exact: true });
    await expect(confirmButton).toBeVisible({ timeout: 5_000 });
    await confirmButton.click();

    // The delete invoke reached the (mocked) backend.
    await expect.poll(() => callLogIncludes(page, 'api_delete_meeting'), {
      timeout: 15_000,
    }).toBe(true);

    // ── The banner is gone ─────────────────────────────────────────────────
    await expect(banner).toBeHidden({ timeout: 15_000 });

    // ── The IndexedDB mirror row is gone ───────────────────────────────────
    // The banner clearing is driven by the mock's job-less snapshot; this
    // probe is what actually pins the NEW frontend line of the fix
    // (handleDelete → removeQueueJob). It reads the real, unmocked IndexedDB
    // from the page. Row removal is awaited after the delete emit, so poll.
    // Schema per indexedDBService: DB 'MeetilyRecoveryDB' v2, store
    // 'transcription_queue', keyed by meetingId.
    await expect.poll(async () => {
      return page.evaluate(async (meetingId) => {
        const db = await new Promise<IDBDatabase>((resolve, reject) => {
          const req = indexedDB.open('MeetilyRecoveryDB', 2);
          req.onsuccess = () => resolve(req.result);
          req.onerror = () => reject(req.error);
        });
        try {
          const rows = await new Promise<unknown[]>((resolve, reject) => {
            const tx = db.transaction(['transcription_queue'], 'readonly');
            const getAll = tx.objectStore('transcription_queue').getAll();
            getAll.onsuccess = () => resolve(getAll.result as unknown[]);
            getAll.onerror = () => reject(getAll.error);
          });
          return rows.some((r) => (r as { meetingId?: string }).meetingId === meetingId);
        } finally {
          db.close();
        }
      }, SMOKE_MEETING_ID);
    }, { timeout: 10_000 }).toBe(false);

    // No uncaught page errors anywhere in the round-trip.
    expect(pageErrors).toEqual([]);
  });
});
