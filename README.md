# StatusLine
> Version 1.1.3

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
📁 project_dir › 🌿 feat-008a [+2|-1] │ Opus 4.7 · effort: high │ 96k/200k (▓▓▓▓▓░░░░░ 48%) · Cache 94% 56:41 · 5h: ▓▓░░░░░░░░ 20% @15:00 · 7d: ▓▓▓▓▓░░░░░ 50% @Apr 24, 08:00 · F5: ▓▓▓░░░░░░░ 30% @Apr 24, 08:00 · extra: $1.23/$10.00
```

When the line exceeds the terminal width, it wraps:
```
📁 project_dir › 🌿 feat-008a [+2|-1] │ Opus 4.7 · effort: high
└─ 96k/200k (▓▓▓▓▓░░░░░ 48%) · Cache 94% 56:41 · 5h: ▓▓░░░░░░░░ 20% @15:00 · 7d: ▓▓▓▓▓░░░░░ 50% @Apr 24, 08:00 · F5: ▓▓▓░░░░░░░ 30% @Apr 24, 08:00 · extra: $1.23/$10.00
```

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

Dot mode always uses 10 positions: `●` is used and dim `○` is available. The row color communicates utilization: green below 50%, orange at 50–69%, yellow at 70–89%, and red at 90% or above. Any value other than `dots` falls back to `bar`.

## Fable weekly usage

`F5` is shown only when the Anthropic OAuth usage response contains a matching `limits[]` entry with `kind: "weekly_scoped"` and `scope.model.display_name: "Fable"`. Claude Code's built-in rate-limit input does not include per-model weekly limits, so the script reuses its secured OAuth credentials and 60-second usage cache to retrieve this optional value. Missing credentials, network failures, or accounts without a Fable scope leave the block hidden.

### Cache block

`Cache <hit%> <MM:SS>` shows cache hit rate and 1-hour TTL countdown:
- Hit rate = `cache_read / (input + cache_creation + cache_read)`; green ≥50%, gray <50%
- TTL counts down from the last response; only resets when the token signature changes (so the timer doesn't restart on every prompt redraw)
- Color stages: 0–20m green / 20–40m yellow / 40–55m red / last 5m blinking red / expired `exp` gray

## Requirements

- `bash` 4+, `jq` 1.6+, `curl`, `git`
- `timeout` (GNU coreutils) optional — without it the git diff DoS guard falls back to unwrapped invocation. macOS users can install via `brew install coreutils` (provides `gtimeout`)

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
