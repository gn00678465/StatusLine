# Config theme: layout, meter, colors

Status: ready-for-agent

## Why

The limits block (`5h · 7d · <weekly scope>`) sits at the end of the detail line. In a narrow or split pane Claude Code truncates it. Users also want to tune meter appearance and colors. All new knobs live only in the config file; no new env vars.

## Config file schema (additions)

```toml
layout = "stacked"          # "auto" (default, current behavior) | "stacked"

[meter]
width = 10                  # 1..=20, default 10
filled = "▓"                # default depends on usage_style: bar "▓", dots "●"
empty = "░"                 # default depends on usage_style: bar "░", dots "○"
show_percentage = true      # false drops the " 25%" after every meter
show_reset = true           # false drops the " @14:00" reset suffix on limit blocks

[colors]
folder = "#64DCFF"          # "#RRGGBB"; defaults are the current palette in src/render/color.rs
branch = "#64FF64"
model = "#50B4FF"
tokens = "#F0F0F0"
levels = ["#64FF64", "#FFE650", "#FFAA50", "#FF6464"]  # meter colors below t0, t0..t1, t1..t2, >= t2
thresholds = [50, 70, 90]   # strictly ascending, each <= 100
```

Every key is optional. An absent key keeps today's value.

## Data shape

Parse at the boundary, render from a typed value. One `Theme` built once in `Config`, passed by reference to `render` and `render_limits`, replacing the loose `meter_style` fields.

```rust
pub struct Theme {
    pub layout: Layout,          // enum Layout { Auto, Stacked }
    pub meter: MeterTheme,
    pub colors: Palette,
}
pub struct MeterTheme {
    pub style: MeterStyle,       // Bar | Dots, from usage_style (env > file > default, unchanged)
    pub width: usize,
    pub filled: String,
    pub empty: String,
    pub show_percentage: bool,
    pub show_reset: bool,
}
pub struct Palette {
    pub folder: Rgb, pub branch: Rgb, pub model: Rgb, pub tokens: Rgb,
    pub levels: Option<[Rgb; 4]>,   // None keeps the per-style default ladders
    pub thresholds: [u8; 3],
}
pub struct Rgb(u8, u8, u8);          // renders as \x1b[38;2;R;G;Bm
```

Meter color is a lookup: level index from `thresholds`, color from `levels` or the default ladder for the style. Default ladders keep v1 parity exactly: bar `[GREEN, YELLOW, ORANGE, RED]`, dots `[GREEN, ORANGE, YELLOW, RED]`; the extra-usage block keeps the bar ladder when `levels` is unset.

## Validation

New keys are typed at deserialize time (serde `try_from` / enum rename). A wrong type, a bad `#RRGGBB`, `width` outside `1..=20`, non-ascending or `> 100` thresholds, or an empty / control-character glyph is a parse error, handled by the existing path: the whole file is ignored and one stderr diagnostic line is printed. `usage_style` and `git_cache_ttl` keep their current lenient parsing and env precedence.

## Stacked layout

`layout = "stacked"` always renders three lines:

```
📁 project › 🌿 main │ 🤖 Opus · 🧠 xhigh
├─ ⚡️ 50k/200k (▓▓░░░░░░░░ 25%) · Cache 50% 60:00
└─ 📊 5h: ▓▓▓░░░░░░░ 30% @14:00 · 7d: … · Fable: …
```

Line 2 holds context and cache; line 3 holds limits. The prefixes are `├─ ` and `└─ ` with the same DIM styling as today's `└─`. `auto` keeps the current width-based two-line behavior byte-for-byte.

## Must hold

- No config file, or a file with only the old keys: stdout byte-identical to today (existing snapshots unchanged).
- Render path still always prints at least `Claude` and exits 0.
- Colors not covered by this schema (effort, git S/W/C, cache TTL, DIM separators) stay fixed.

## Acceptance

- Unit tests per knob against literal expected strings.
- Integration test in `tests/cc_statusline.rs` running the real binary with a config file setting `layout = "stacked"` plus a meter and color override.
- README "Config file" section and CHANGELOG `[Unreleased]` document the new keys.

## Revision 2: width-aware `auto` (measured 2026-09-26)

### Measured facts

Probe script installed as `statusLine.command` in an Orca split pane, output read back with `orca terminal read`:

- Claude Code sets `COLUMNS` to the real pane width on every status-line run, and it follows resizes (78, then 94 after a resize; both equal `stty size` of the pane tty). stdin JSON carries no width or padding field. `/dev/tty` is not available.
- Claude Code reserves 2 columns on each side plus `statusLine.padding` on each side. Usable width = `COLUMNS - 4 - 2 * padding` (94 cols, padding 2 → 86 fits, 87 is cut; padding 0 → 90 fits, 91 is cut).
- A line wider than that is cut at the end and its last visible cell becomes `…`.

Revision 1's premise ("the binary cannot see the pane width") was wrong; the README and CHANGELOG text built on it is replaced.

### Config

```toml
padding = 2   # top-level; must equal statusLine.padding in Claude Code settings; 0..=20, default 0
```

Invalid (`> 20`, negative, wrong type) follows the existing whole-file-ignore path.

### `auto` layout (max 3 lines, user decision)

`budget = COLUMNS.saturating_sub(4 + 2 * padding)`, computed once. With `inline = header + " │ " + detail` and `detail = context · cache · limits`:

1. `width(inline) <= budget` → 1 line (unchanged shape).
2. else `3 + width(detail) <= budget` → 2 lines: `header` / `└─ detail` (unchanged shape).
3. else 3 lines, identical to `stacked`: `header` / `├─ context · cache` / `└─ limits`.

Blocks never move further down; a line that still overflows is left for Claude Code to cut. `stacked` is unchanged. `width.rs::wrap_status_line` goes away if nothing else uses it.
