# Project context — fed into every OpenSpec proposal

Canonical rulebook: [`AGENTS.md`](../AGENTS.md) (repo root). This file exists
so proposal-time tooling and agents have the project-specific constraints in
one repo-owned place that survives plugin/tool updates — do not duplicate the
rulebook here; read it.

Non-negotiables that most often bind a proposal:

- **Spec-driven workflow** — AGENTS.md §3: proposal → design → tasks (each
  task "write the failing test, then make it pass") → implement → archive
  against the canonical spec.
- **UI-facing changes carry an E2E smoke spec** — AGENTS.md §3 + §4
  (Frontend / UI category): any change whose outcome users see or trigger,
  including backend-only commands behind a badge, button, or dialog, lands
  `frontend/e2e/smoke/<change-name>.spec.ts` as an explicit tasks.md
  deliverable, or records in tasks.md why the surface is already pinned.
- **Adversarial TDD categories** — AGENTS.md §4: at least one red test per
  applicable category BEFORE implementation.
- **Test gates** — AGENTS.md §5: `cargo test` (Rust), `pnpm test` (Vitest),
  Playwright smoke specs; the pre-push hook runs the full E2E suite on
  main-direct pushes.
