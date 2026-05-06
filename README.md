# StatusLine
> Version 1.1.2

A Claude Code status line script with token usage, cache hit rate, TTL countdown, and 5h/7d/extra rate limits.

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
📁 project_dir › 🌿 feat-008a [+2|-1] │ Opus 4.7 · effort: high │ 96k/200k (▓▓▓▓▓░░░░░ 48%) · Cache 94% 56:41 · 5h: ▓▓░░░░░░░░ 20% @15:00 · 7d: ▓▓▓▓▓░░░░░ 50% @Apr 24, 08:00 · extra: $1.23/$10.00
```

When the line exceeds the terminal width, it wraps:
```
📁 project_dir › 🌿 feat-008a [+2|-1] │ Opus 4.7 · effort: high
└─ 96k/200k (▓▓▓▓▓░░░░░ 48%) · Cache 94% 56:41 · 5h: ▓▓░░░░░░░░ 20% @15:00 · 7d: ▓▓▓▓▓░░░░░ 50% @Apr 24, 08:00 · extra: $1.23/$10.00
```

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
