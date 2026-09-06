# StatusLine

`cc-statusline`: a single Rust binary rendering the Claude Code status line (v2, released via GitHub Releases). The bash predecessor lives at tag `v1-final`.

## Working on this repo

- **v1 parity is the reference semantics.** When behavior is ambiguous, the shell implementation at `v1-final` is the authority; deliberate divergences are whitelisted in `.scratch/rust-rewrite/parity-whitelist.md`.
- **External effects go behind trait seams** (`GitRunner`, `HttpClient`, `CredentialStore`, `Clock`, `LocalOffset`) so tests inject mocks; no test touches network, keychain, or wall-clock time.
- **New dependencies need a stated justification** — the crate deliberately stays near-zero-dep (see spec §5).
- Render path must always print at least `Claude` and exit 0 — a blank or non-zero statusline disappears in Claude Code.
- Decision record: spec and tickets in `.scratch/rust-rewrite/`; the *why* behind architecture and distribution choices (napi-rs rejection, npm 12 / pnpm / bun install-script policies) is in `docs/research/`.

## Autonomy and approval

These rules apply only to work in this repository. They override conflicting shared skill and workflow clauses, not the rest of those workflows. Do not modify shared skills or configuration outside this repository as part of a project task.

- Carry authorized work through implementation, relevant checks, and delivery. Choose the simplest implementation that satisfies all explicit requirements; reducing requested scope requires agreement.
- Reuse explicit authorization from the current task while its scope remains applicable. Distinguish permission to act from the durable evidence needed to certify the result: missing spec paperwork does not invalidate existing authorization, but does not count as spec approval either.
- Resolve questions from repository evidence first. Ask only when an unresolved choice materially affects scope, observable behavior, acceptance criteria, risk, or authorization. Record reasonable assumptions for routine choices and continue independent work while awaiting a necessary answer. Use an interview workflow only when requested.
- Preserve explicit approval requirements for initial evidence-first specs, material spec changes, and operations not already authorized. Prepare a concrete proposal before requesting approval. A clarification answer or lack of response is not approval. A new dependency still needs a stated justification; that requirement alone does not add an approval step.

## Evidence and completion

- Apply evidence-first when explicitly requested or when a change affects money, authorization, security, data preservation, concurrency correctness, or an external API contract. A filename, directory, or textual mention alone does not trigger it. Identify the affected behavior or contract; routine changes use relevant existing checks.
- Keep initial spec approval and its durable record. Re-approval is required when scope, observable behavior, acceptance criteria, Must NOT constraints, risk, or authorized operations change. Editorial corrections that change none of these retain the approved `spec_version`; log the correction without replacing the approval record. Material revisions get a new version and approval. Never relabel stale evidence to match a revised spec.
- Reuse authorized verification setup and existing checks. Ask before adding tools or write scope that has not been authorized. If durable intent is missing, run independent executable checks and report the evidence limitation; do not invent approval or repeatedly ask for permission already granted.
- Autonomous execution without spec approval requires an explicitly designated unattended task; elapsed waiting time does not enable it. Such a run may deliver authorized implementation and checks with an approval downgrade, but must not mark the spec `approved` or `shipped`, or claim CLOSE completed.
- New or worsened failures, and existing failures that prevent this task's acceptance checks, block completion. Record unrelated failures reproduced on the baseline without expanding the repair scope; distinguish completion of the requested change from a fully passing gate.
- On a tool refusal, preserve the reason, repair safely recoverable prerequisites within existing authorization, and rerun the original tool. Do not bypass checks, fabricate approval, or alter unrelated user changes. If blocked, report implementation, verification, approval, and archival status separately; partial delivery is not full completion.

## Commits

- When a commit is requested on the main branch, create a local branch with an available semantic name and continue, unless the user restricts branch operations. Preserve the working tree and staged content. This does not authorize pushing, merging, or committing directly to the main branch.
- Commit complexity scores determine review depth, not whether work stops. Split only along independently understandable, buildable, and revertible boundaries; keep inseparable changes together. Execute the authorized commits rather than returning instructions for the user to execute. Preserve any required RED-before-GREEN evidence sequence.
- For a staged-content commit request, keep the original staged scope. Do not include unstaged content; ask if splitting partial staging cannot safely preserve that scope.

## Agent skills

### Issue tracker

Issues live as local markdown files under `.scratch/<feature-slug>/` in this repo. See `docs/agents/issue-tracker.md`.

### Triage labels

Default canonical labels, recorded as `Status:` lines in issue files. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context layout: one `CONTEXT.md` plus `docs/adr/` at the repo root. See `docs/agents/domain.md`.
