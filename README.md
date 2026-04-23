# StatusLine
> Version 1.0.0

Copy the `claudeStatusLine.sh` file to `~/.claude`, and add the following settings to `settings.json`:
```
{
    "statusLine": {
        "type": "command",
        "command": "~/.claude/claudeStatusLine.sh"
    },
}
```

**Example**
```
 📁 project_dir › 🌿 feat-008a [+2|-1] │ Opus 4.7 · effort: high
 └─ 96k/200k (▓▓▓▓▓░░░░░ 48%) · 5h: ▓▓░░░░░░░░ 20% @15:00 · 7d: ▓▓▓▓▓░░░░░ 50% @Apr 24 · extra: enabled
```