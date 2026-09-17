# AGENTS.md

## Openspec changes touching UI — E2E smoke spec is part of the change

Any openspec change that adds or modifies behavior surfaced through the UI —
frontend components/views AND backend commands whose outcome users see or
trigger in the app (badges, buttons, dialogs, transcript rendering, speaker
labeling, …) — SHALL include a Playwright smoke spec under
`frontend/e2e/smoke/` named after the change, driving the real user flow
(click → command dispatch → refetch → visible outcome) through the fail-closed
Tauri mock (`e2e/mocks/init-script.ts`; per-page bootstrap helpers under
`e2e/smoke/`).

- The E2E decision is made at proposal time and recorded in the change's
  `tasks.md`: either a task lands the spec, or a task records WHY the UI
  surface is already pinned elsewhere (link the covering spec). "Considered
  it" without a recorded decision does not satisfy this rule.
- Backend/unit tests do not substitute: they pin logic, not the wiring users
  depend on (affordance renders → dispatch carries the right args → refetch
  re-renders the outcome).
- E2E specs that depend on backend state change the fixture inside the mock's
  command handler (mirroring exactly what the real backend persists), then
  assert the visible result after refetch — see
  `e2e/smoke/recognized-name-badge-revert.spec.ts` as the pattern.
- Run the spec before archiving the change (`npx playwright test
  e2e/smoke/<spec>.spec.ts` from `frontend/`), in addition to the Rust/vitest
  suites.
