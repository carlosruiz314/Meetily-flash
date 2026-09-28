import { test, expect } from '@playwright/test';
import {
  TAURI_MOCK_INIT_SCRIPT,
  TAURI_INTERNALS_SPY_INIT_SCRIPT,
} from '../mocks/init-script';

// Task 2.3 — proves the webpack alias swapped @tauri-apps/api/core for the
// fixture-backed mock. The app's own `invoke` import routes through the mock
// module to window.__tauriMockDispatcher, never touching __TAURI_INTERNALS__.

test.describe('module-seam interception (2.3)', () => {
  test.beforeEach(async ({ page }) => {
    // Spy first so its getter is in place before any app script runs.
    await page.addInitScript(TAURI_INTERNALS_SPY_INIT_SCRIPT);
    await page.addInitScript(TAURI_MOCK_INIT_SCRIPT);
    await page.goto('/');
    // Wait for the mock module to have loaded (marker set at module-eval time).
    await page.waitForFunction(
      () => (window as unknown as { __tauriCoreMockActive?: boolean }).__tauriCoreMockActive === true,
      { timeout: 15_000 },
    );
  });

  test('invoke returns the fixture response registered on the dispatcher', async ({ page }) => {
    await page.evaluate(() => {
      (window as unknown as {
        __tauriMockDispatcher: { register: (cmd: string, fn: (a: unknown) => unknown) => void };
      }).__tauriMockDispatcher.register('start_recording', () => ({
        ok: true,
        meeting_id: 'meet-seam-001',
      }));
    });

    const result = await page.evaluate(() =>
      (window as unknown as { __tauriMockInvoke: (cmd: string, args?: unknown) => Promise<unknown> })
        .__tauriMockInvoke('start_recording', { meeting_name: 'Seam Test' }),
    );

    expect(result).toEqual({ ok: true, meeting_id: 'meet-seam-001' });
  });

  test('the mock never accesses window.__TAURI_INTERNALS__ during an invoke call', async ({ page }) => {
    // Register, reset, invoke and read all run inside ONE evaluate: the
    // assertion window is exactly the invoke itself. The original four
    // separate evaluates left gaps where the still-hydrating app's timers
    // could touch internals between reset and read — a false positive that
    // only fired under full-suite CPU load.
    const count = await page.evaluate(async () => {
      const w = window as unknown as {
        __tauriMockDispatcher: { register: (cmd: string, fn: (a: unknown) => unknown) => void };
        __resetTauriInternalsSpy: () => void;
        __tauriMockInvoke: (cmd: string) => Promise<unknown>;
        __tauriInternalsAccessCount: () => number;
      };
      w.__tauriMockDispatcher.register('ping', () => 'pong');
      w.__resetTauriInternalsSpy();
      await w.__tauriMockInvoke('ping');
      return w.__tauriInternalsAccessCount();
    });
    expect(count).toBe(0);
  });
});
