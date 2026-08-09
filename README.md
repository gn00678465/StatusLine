# StatusLine
> Version 1.2.0

A Claude Code status line script with token usage, cache hit rate, TTL countdown, and 5h/7d/Fable weekly/extra rate limits.

Copy `claudeStatusLine.sh` to `~/.claude`, then add to `settings.json`:

**Linux / macOS:**
```json
{
  "statusLine": {
    "type": "command",
    "command": "~/.claude/claudeStatusLine.sh",
    "padding": 2
  }
}
```

**Windows (Git Bash):**
```json
{
  "statusLine": {
    "type": "command",
    "command": "bash /c/Users/<USER>/.claude/claudeStatusLine.sh",
    "padding": 2
  }
}
```

## Example

```
📁 project_dir › 🌿 feat-008a [S2|W1] │ 🤖 Opus 4.7 · 🧠 high │ ⚡️ 96k/200k (▓▓▓▓▓░░░░░ 48%) · Cache 94% 56:41 · 📊 5h: ▓▓░░░░░░░░ 20% @15:00 · 7d: ▓▓▓▓▓░░░░░ 50% @Apr 24, 08:00 · Fable 5: ▓▓▓░░░░░░░ 30% @Apr 24, 08:00 · extra: $1.23/$10.00
```

When the line exceeds the terminal width, it wraps:
```
📁 project_dir › 🌿 feat-008a [S2|W1] │ 🤖 Opus 4.7 · 🧠 high
└─ ⚡️ 96k/200k (▓▓▓▓▓░░░░░ 48%) · Cache 94% 56:41 · 📊 5h: ▓▓░░░░░░░░ 20% @15:00 · 7d: ▓▓▓▓▓░░░░░ 50% @Apr 24, 08:00 · Fable 5: ▓▓▓░░░░░░░ 30% @Apr 24, 08:00 · extra: $1.23/$10.00
```

The semantic icons identify each information group without changing the existing layout: `🤖` model, `🧠` effort, `📁` workspace, `🌿` Git, `⚡️` context window, and one `📊` for the complete rate-limit group. The vocabulary is informed by [`best-claude-hud`](docs/research/best-claude-hud-layout-and-icons.md); this script keeps its own wrapping and meter UI.

## Git status

The Git suffix reports changed **files**, not diff lines: `S` is staged/index, `W` is unstaged working-tree state, and `C` is conflict. A file staged and then edited again appears in both counts, for example `[S1|W1]`. Untracked files, recursive submodule dirtiness, rename similarity, and ahead/behind are deliberately skipped so a frequently refreshed status line does not perform an expensive repository scan.

One `git status --porcelain=v2` call supplies both branch and state. It runs with `--no-optional-locks`, so the background query does not refresh/write the index or contend with foreground Git commands. Results use a 2-second per-session cache to absorb redraw bursts; set `STATUSLINE_GIT_CACHE_TTL=0` to disable it, or an integer up to 60 to change the TTL. See the [reference-project analysis](docs/research/git-status-stage-performance.md) for the command and trade-offs.

## Usage meter style

The default `bar` style preserves the existing horizontal `▓░` meters. Set one environment variable to switch context, 5h, 7d, and Fable weekly usage together:

```json
{
  "statusLine": {
    "type": "command",
    "command": "STATUSLINE_USAGE_STYLE=dots ~/.claude/claudeStatusLine.sh",
    "padding": 2
  }
}
```

On Windows with Git Bash, use:

```json
"command": "STATUSLINE_USAGE_STYLE=dots bash /c/Users/<USER>/.claude/claudeStatusLine.sh"
```

Dot mode always uses 10 positions separated by one space: `● ● ● ○ ○ ○ ○ ○ ○ ○`. The default bar remains contiguous: `▓▓▓░░░░░░░`. In dot mode, `●` is used and dim `○` is available. The row color communicates utilization: green below 50%, orange at 50–69%, yellow at 70–89%, and red at 90% or above. Any value other than `dots` falls back to `bar`.

## Fable weekly usage

`Fable 5` is shown only when the Anthropic OAuth usage response contains a matching `limits[]` entry with `kind: "weekly_scoped"` and `scope.model.display_name: "Fable"`. Claude Code's built-in rate-limit input does not include per-model weekly limits, so the script reuses its secured OAuth credentials and 60-second usage cache to retrieve this optional value. Missing credentials, network failures, or accounts without a Fable scope leave the block hidden.

### Cache block

`Cache <hit%> <MM:SS>` shows cache hit rate and 1-hour TTL countdown:
- Hit rate = `cache_read / (input + cache_creation + cache_read)`; green ≥50%, gray <50%
- TTL counts down from the last response; only resets when the token signature changes (so the timer doesn't restart on every prompt redraw)
- Color stages: 0–20m green / 20–40m yellow / 40–55m red / last 5m blinking red / expired `exp` gray

## Requirements

- `bash` 4+, `jq` 1.6+, `curl`, `git`
- `timeout` (GNU coreutils) optional — without it Git and keychain lookups fall back to unwrapped invocation. macOS users can install via `brew install coreutils` (provides `gtimeout`)

## Security

Hardened across six review rounds. See [CHANGELOG.md](CHANGELOG.md) for details. Threat-model summary:
- Single-user trusted shell: production-ready
- Multi-user shared host (classic Unix mode bits): production-ready
- Multi-user shared host with extended POSIX/NFSv4 ACLs: operator must verify `$HOME` and `$XDG_RUNTIME_DIR` have no extended write ACLs (`_dir_is_safe()` only checks mode bits)

Cache files live under `${XDG_RUNTIME_DIR}/StatusLine` or `${HOME}/.cache/StatusLine` (mode 700, atomic writes).

## Tests

Run the deterministic mock evaluation without real credentials or network access:

```bash
bash tests/test-statusline.sh
```
