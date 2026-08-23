# StatusLine

`cc-statusline`: a single Rust binary rendering the Claude Code status line (v2, released via GitHub Releases). The bash predecessor lives at tag `v1-final`.

## Working on this repo

- **v1 parity is the reference semantics.** When behavior is ambiguous, the shell implementation at `v1-final` is the authority; deliberate divergences are whitelisted in `.scratch/rust-rewrite/parity-whitelist.md`.
- **External effects go behind trait seams** (`GitRunner`, `HttpClient`, `CredentialStore`, `Clock`, `LocalOffset`) so tests inject mocks; no test touches network, keychain, or wall-clock time.
- **New dependencies need a stated justification** — the crate deliberately stays near-zero-dep (see spec §5).
- Render path must always print at least `Claude` and exit 0 — a blank or non-zero statusline disappears in Claude Code.
- Decision record: spec and tickets in `.scratch/rust-rewrite/`; the *why* behind architecture and distribution choices (napi-rs rejection, npm 12 / pnpm / bun install-script policies) is in `docs/research/`.

## Agent skills

### Issue tracker

Issues live as local markdown files under `.scratch/<feature-slug>/` in this repo. See `docs/agents/issue-tracker.md`.

### Triage labels

Default canonical labels, recorded as `Status:` lines in issue files. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context layout: one `CONTEXT.md` plus `docs/adr/` at the repo root. See `docs/agents/domain.md`.
