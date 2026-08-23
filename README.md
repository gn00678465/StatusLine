# cc-statusline

Version **2.0.0** is a single Rust binary for the Claude Code status line. It
keeps the v1 layout while removing the runtime dependency on shell, `jq`, and
network command-line tools. A redraw is safe to run at 1 Hz: cached work stays
local, and network/keychain refreshes are bounded and fail closed.

## Install

All release assets are published on GitHub Releases. The three supported
installation paths end at `~/.claude/cc-statusline/` (Windows uses the same
directory under `%USERPROFILE%` and installs `cc-statusline.exe`).

### npm installer

```sh
npm install -g https://github.com/gn00678465/StatusLine/releases/latest/download/cc-statusline-npm.tgz
```

The package is an installer, not a JavaScript runtime wrapper. Its postinstall
script selects the native platform asset, verifies its `.sha256` sidecar with
Node's built-in `crypto`, and extracts the binary. `npm install --ignore-scripts`
intentionally does not install a binary; the package prints a manual-install
hint in that case.

### POSIX installer

```sh
curl -fsSL https://github.com/gn00678465/StatusLine/releases/latest/download/install.sh | sh
```

`install.sh` supports `curl` or `wget`, verifies the matching SHA-256 sidecar,
and installs the archive selected by `uname`. A download or verification error
prints the release URL and the manual installation directory.

### Manual download

Choose the asset for the host, download its checksum sidecar, and verify before
extracting:

```sh
asset=cc-statusline-darwin-arm64.tar.gz   # or the matching release asset
base=https://github.com/gn00678465/StatusLine/releases/latest/download
curl -fsSLO "$base/$asset"
curl -fsSLO "$base/$asset.sha256"
sha256sum -c "$asset.sha256"              # macOS: shasum -a 256 -c "$asset.sha256"
mkdir -p "$HOME/.claude/cc-statusline"
tar -xzf "$asset" -C "$HOME/.claude/cc-statusline"
```

Windows users download `cc-statusline-win32-x64.zip` or
`cc-statusline-win32-arm64.zip`, verify the sidecar with a SHA-256 tool, and
extract `cc-statusline.exe` into `%USERPROFILE%\.claude\cc-statusline\`.

## Claude Code settings

Point `statusLine.command` at the installed binary. `refreshInterval: 1` is the
recommended 1 Hz redraw cadence for the warm-cache path:

```json
{
  "statusLine": {
    "type": "command",
    "command": "~/.claude/cc-statusline/cc-statusline",
    "refreshInterval": 1
  }
}
```

On Windows use `~/.claude/cc-statusline/cc-statusline.exe` (or the equivalent
expanded `%USERPROFILE%` path accepted by the Claude Code host).

## Output

The line contains workspace/Git, model/effort, context, cache TTL, and rate
limits. A long line wraps once with `└─`. The `bar` and `dots` meters use the
same ten positions and color thresholds across context and limits.

```text
📁 project › 🌿 feat/status [S1|W2] │ 🤖 Opus 4.7 · 🧠 high │ ⚡️ 96k/200k (▓▓▓▓▓░░░░░ 48%) · Cache 94% 56:41 · 📊 5h: ▓▓░░░░░░░░ 20% @15:00 · 7d: ▓▓▓▓▓░░░░░ 50% @Apr 24, 08:00 · Team model: ▓▓▓░░░░░░░ 30% @Apr 24, 08:00 · extra: $1.23/$10.00
```

### Configuration

| Variable | Values | Default / meaning |
| --- | --- | --- |
| `STATUSLINE_USAGE_STYLE` | `bar` or `dots` | `bar`; other values fall back to `bar` |
| `STATUSLINE_GIT_CACHE_TTL` | `0`–`60` | `2` seconds; controls per-session Git refresh |
| `CLAUDE_CONFIG_DIR` | directory | Credential and OAuth cache configuration directory |
| `CLAUDE_CODE_OAUTH_TOKEN` | token | Highest-priority OAuth token source |
| `COLUMNS` | positive integer | Terminal width; invalid or missing values use `100` |

OAuth credentials are tried in this order: environment token, macOS Keychain,
`.credentials.json`, and (on Linux) `secret-tool`. A missing credential,
network failure, timeout, or malformed response simply hides the optional OAuth
blocks. The token is never placed in a child-process argument list.

### Weekly scoped limits

Claude Code's stdin contains built-in 5-hour and 7-day windows, but not
per-model weekly scopes. When OAuth data is available, every response entry
whose `kind` is `weekly_scoped` is rendered in response order using its
`scope.model.display_name`. Zero entries hide the section; one or many entries
are handled identically. There is no model-specific special case.

## Performance

The benchmark measures the release binary itself, with stdin supplied by
hyperfine (`--shell=none --input`) so shell startup is not included. Both runs
used macOS arm64 on 2026-08-23, `cargo build --release`, `hyperfine` 1.21.0,
an isolated mode-700 `XDG_RUNTIME_DIR`, `CLAUDE_CONFIG_DIR`,
`STATUSLINE_GIT_CACHE_TTL=60`, `COLUMNS=100`, and the complete fixture with its
`cwd` set to this Git checkout:

```sh
cargo build --release
XDG_RUNTIME_DIR="$BENCH_RUNTIME" \
CLAUDE_CONFIG_DIR="$BENCH_CONFIG" \
STATUSLINE_GIT_CACHE_TTL=60 COLUMNS=100 \
hyperfine --shell=none --input "$BENCH_FIXTURE" \
  --warmup 5 --runs 30 target/release/cc-statusline

# Cold case: move the usage-cache file aside, keep the release cache fresh,
# then run the same hyperfine command (macOS invokes `security` for the lookup).
mv "$BENCH_USAGE_CACHE" "$BENCH_USAGE_CACHE.disabled"
XDG_RUNTIME_DIR="$BENCH_RUNTIME" CLAUDE_CONFIG_DIR="$BENCH_CONFIG" \
STATUSLINE_GIT_CACHE_TTL=60 COLUMNS=100 \
hyperfine --shell=none --input "$BENCH_FIXTURE" \
  --warmup 5 --runs 30 target/release/cc-statusline
```

The warm-cache run had fresh OAuth, Git, and update cache entries and measured
**1.7 ± 0.1 ms** (about 2 ms, the ~2.3 ms 1 Hz target). For the cold-cache
run, the OAuth cache entry was removed while the release cache stayed fresh;
the macOS `security` lookup therefore ran in the child-process timeout path.
It measured **11.4 ± 0.3 ms** (about 12 ms). The exact output ranges were
1.6–2.0 ms and 10.7–12.2 ms respectively.

## Requirements

The render-time binary has **zero external runtime dependencies**. `git` is
optional: without it, the workspace block gracefully degrades. Runtime use
does not require `jq`, `curl`, `bash`, Node.js, or GNU `timeout`.

The npm installer needs Node.js 18 or newer; the POSIX installer needs a POSIX
shell plus either `curl`/`wget` and a SHA-256 utility. These are installer
tools only and are not runtime requirements.

## Security model

The threat model follows the hardened v1 implementation: a single trusted user
is production-ready, and a multi-user Unix host is supported when traditional
mode bits are authoritative. Extended POSIX/NFSv4 ACLs remain an operator
responsibility because mode-bit inspection cannot prove that an ACL grants no
additional write access.

Cache selection tries `$XDG_RUNTIME_DIR` first and then `$HOME/.cache`. Every
ancestor and the `StatusLine` directory must be a user-owned, non-symlink
directory without group/other write bits; failure is fail-closed and disables
disk cache access. New directories are created private (`0700`) at creation
time. Cache files use private temporary files plus `rename` for atomic writes;
mkdir locks have a bounded stale lifetime. Reads reject symlinks.

Payload strings, cached API responses, and release tags are sanitized before
terminal output. OAuth tokens stay in process memory and HTTP headers; they are
never passed in `security`, `secret-tool`, or any other child-process argv.

## Development and tests

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
(cd npm && npm test)
sh tests/test-install.sh
```

The Rust tests read the checked-in `tests/fixtures/*.json` files. Network,
credential, Git, clock, and local-time seams are injected in tests, so the
commands above do not require a Claude session or OAuth credentials.
