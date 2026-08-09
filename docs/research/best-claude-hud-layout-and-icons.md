# `best-claude-hud` 布局與 emoji/icon 研究

## 研究範圍

使用者提供的本機 clone 位於 `/d/Projects/status-line-dev/best-claude-hud`；其 `origin` 是 [`GaoSSR/best-claude-hud`](https://github.com/GaoSSR/best-claude-hud)，不是名稱相近的 `jarrodwatts/claude-hud`。本文件只使用該 clone 的 README、原始碼、changelog、preview 與官方 GitHub Release；觀察固定於：

- tag：[`v0.1.11`](https://github.com/GaoSSR/best-claude-hud/releases/tag/v0.1.11)
- commit：[`4e6228bfeea1215e66da85def500e348808458b6`](https://github.com/GaoSSR/best-claude-hud/tree/4e6228bfeea1215e66da85def500e348808458b6)
- 發布日期：2026-08-08

本機 clone 因換行格式顯示大量 modified，但針對本研究引用的檔案執行 `git diff --ignore-space-at-eol --exit-code` 為 0；下列程式語意與固定的 HEAD blob 相同。研究前亦已完整讀取該 clone 的 `AGENTS.md`；其額外規則只適用於 release work，本研究沒有修改或發布該專案。

另外下載官方 `best-claude-hud-linux-x64-musl.tar.gz`，其 SHA-256 `25867fa8518ce0d487a9c0dc0da72c6ea405632579de985edd45a563391eabb8` 與 Release 附帶的 checksum 相符，再以 mock stdin 驗證實際輸出。Release 資產來源：[`v0.1.11`](https://github.com/GaoSSR/best-claude-hud/releases/tag/v0.1.11)。

## 結論摘要

1. `best-claude-hud` 的 live HUD 是一條由 segments 組成的單行：預設順序為 model（內含 effort）→ directory → git → context window；usage、cost、session、output style 排在其後但預設關閉。[主題順序與 separator](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/src/ui/themes/presets.rs#L145-L181)
2. 預設 Plain 主題的主要 emoji 語意是 `🤖` model、`🧠` effort、`📁` directory、`🌿` git、`⚡️` context、`📊` usage、`💰` cost、`⏱️` session、`🎯` output style。[預設主題](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/src/ui/themes/theme_default.rs#L6-L163) [effort renderer](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/src/core/statusline.rs#L259-L287)
3. 上游沒有十格 `●/○` dots，也沒有 `▓/░` bar。Plain usage 永遠使用 `📊`；Nerd Font/Powerline 才以單一八階段 circle-slice glyph 表達 7-day 使用程度。[usage glyph 計算](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/src/core/segments/usage.rs#L31-L48) [mode 選擇](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/src/core/statusline.rs#L220-L229) [v0.1.11 changelog](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/CHANGELOG.md#L3-L9)
4. 因此「dot 之間一個空格、bar glyph 完全相連」不是複製上游規格，而是本專案應明確定義並以測試保護的視覺規則。建議輸出 `● ● ● ○ ○ ○ ○ ○ ○ ○`，但 bar 維持 `▓▓▓░░░░░░░`。

## 1. Live HUD 布局

### 1.1 預設順序與啟用狀態

所有內建主題都以相同順序宣告 segments：

```text
model → directory → git → context_window → usage → cost → session → output_style
```

其中預設主題只啟用前四項；usage、cost、session、output style 的 `enabled` 都是 `false`。來源：[preset 的順序](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/src/ui/themes/presets.rs#L145-L162)、[default theme 啟用狀態](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/src/ui/themes/theme_default.rs#L6-L163)。README 也把預設核心描述為 model/effort、workspace、Git、context，將 usage/rate-limit、cost、session、output style 稱為 optional。[README overview](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/README.md#L29-L43)

effort 不是獨立 segment。當 model 有 secondary value 時，renderer 把它放在 model 文字後面，固定形成：

```text
🤖 <model> | 🧠 <effort>
```

這個內部分隔固定使用 ASCII `|`，周圍各一個空格；Plain 使用 `🧠`，Nerd Font/Powerline 改用對應 brain glyph。來源：[renderer](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/src/core/statusline.rs#L259-L287)、[README effort 說明](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/README.md#L268-L296)。

### 1.2 實際 mock 輸出

對官方 `v0.1.11` Linux binary 傳入以下關鍵 mock 值：Opus 4.1、medium effort、repository path、clean `main`、context 50k/200k（25%）。移除 ANSI color 後的實際輸出是：

```text
🤖 Opus 4.1 | 🧠 medium | 📁 repo | 🌿 main ✓ | ⚡️ 25% · 50k tokens
```

這與上游 preview 圖的可見結構一致：model/effort 在前，再接 folder、git、context。[官方 preview](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/assets/best-claude-hud-preview.png) Preview UI 本身的 mock data 也使用 model、directory、Git clean、context、usage、cost、session 等相同 segment shape。[preview mock source](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/src/ui/components/preview.rs#L84-L205)

### 1.3 分隔、空白與省略

- Default、Cometix、Gruvbox 使用 `" | "`；Minimal 使用 `" │ "`；Nord 的 separator 是空字串；Powerline themes 使用 ``。[全部 preset separator](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/src/ui/themes/presets.rs#L125-L302)
- 非 Powerline separator 統一套白色後，直接 join 已成功 render 的 segments；空資料不會留下孤立或重複 separator。[segment 收集與 join](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/src/core/statusline.rs#L45-L69) [white separator join](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/src/core/statusline.rs#L364-L373)
- 一般 segment 固定為 `icon + 一個空格 + text`；有背景色的 Powerline segment 則在 segment 兩端各補一個空格。[segment renderer](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/src/core/statusline.rs#L220-L256)
- Powerline `` 會以前一段背景色作 foreground、下一段背景色作 background，形成連續色塊過渡。[Powerline join](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/src/core/statusline.rs#L375-L441)

### 1.4 換行規則

正式 statusline path 呼叫 `generate()` 後以一次 `println!` 輸出；`generate()` 只 join segments，沒有讀取 terminal width，也沒有插入第二行。因此上游 live HUD 本身是單行，過長時只能交由 terminal/Claude Code 外層處理。[main output path](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/src/main.rs#L73-L84) [live generator](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/src/core/statusline.rs#L45-L69)

原始碼另有 `generate_for_tui_preview()`，會依 `max_width` 在完整 segment 邊界換行，且換行時不保留行尾 separator；但它只用於 configurator preview，不是 live statusline path。[TUI preview wrapping](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/src/core/statusline.rs#L94-L218)

## 2. Emoji/icon 與語意

### 2.1 Default Plain 主題

| 區塊 | Plain icon | 預設色彩 | 狀態 |
| --- | --- | --- | --- |
| Model | `🤖` | cyan | 啟用 |
| Effort | `🧠` | bright purple `#B45CFF` | 有 effort data 時內嵌於 model |
| Directory | `📁` | icon yellow、文字 green | 啟用 |
| Git | `🌿` | blue | 啟用 |
| Context window | `⚡️` | magenta | 啟用 |
| Usage/rate limit | `📊` | cyan | 預設關閉 |
| Cost | `💰` | yellow | 預設關閉 |
| Session | `⏱️` | green | 預設關閉 |
| Output style | `🎯` | cyan | 預設關閉 |

來源：[Default theme 的 icon、color 與 enabled](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/src/ui/themes/theme_default.rs#L6-L163)、[effort icon/color](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/src/core/statusline.rs#L5-L6)。

Minimal 主題刻意降低 emoji 密度：model `✽`、directory `◐`、git `※`、context `◐`；optional cost/session/output style/usage 仍是 `💰`、`⏱️`、`🎯`、`📊`。[Minimal theme](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/src/ui/themes/theme_minimal.rs#L6-L163)

### 2.2 Git 狀態符號

Git segment 將分支名稱放在 primary，狀態放在 secondary，符號如下：

| 符號 | 語意 |
| --- | --- |
| `✓` | working tree clean |
| `●` | 有未提交變更（dirty） |
| `⚠` | conflict |
| `↑n` | ahead upstream n commits |
| `↓n` | behind upstream n commits |

多個狀態項目用一個 ASCII 空格連接。來源：[Git renderer](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/src/core/segments/git.rs#L174-L213)、[README 指標表](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/README.md#L300-L308)。這裡的 `●` 是「Git dirty」狀態，不是 meter 的 filled dot。

### 2.3 可選 icon catalog

Configurator 的 Plain icon picker 還提供 `💻` computer、`🖥️` desktop、`⚙️` settings、`📂` open folder、`📊` chart、`🌱` seedling、`🔧` wrench、`⚡` usage、`⭐`、`✨`、`🔥`、`💎`、`✓/✗`、`●/○` 與方向三角形等選項。這是可選 icon 清單，不代表預設 HUD 同時使用它們。[Plain icon catalog](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/src/ui/components/icon_selector.rs#L341-L444)

## 3. Meter 呈現規則

### 3.1 上游的實際行為

`best-claude-hud` 不繪製十格 bar 或 dots：

- Context 是文字：`<percent> · <tokens> tokens`。[context renderer](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/src/core/segments/context_window.rs#L23-L85)
- Usage 的文字 primary 是四捨五入後的 5-hour percent，secondary 是 `· <reset>`。[stdin usage renderer](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/src/core/segments/usage.rs#L213-L250)
- Plain mode 固定顯示 configured icon `📊`，不使用進度 glyph；這是 `v0.1.11` 明確修正的行為。[v0.1.11 changelog](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/CHANGELOG.md#L3-L9)
- Nerd Font/Powerline mode才依 7-day utilization 選一個 `circle_slice_1` 到 `circle_slice_8` glyph。區間為 0–12、13–25、26–37、38–50、51–62、63–75、76–87、88–100%；整個 meter 仍只佔一個 glyph。[八階 circle slice](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/src/core/segments/usage.rs#L31-L48)
- 使用色彩來自主題的靜態 segment color，程式沒有依 50/70/90 等 utilization threshold 動態換色。[Default usage color](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/src/ui/themes/theme_default.rs#L82-L110) [usage data](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/src/core/segments/usage.rs#L213-L250)

全 repository 的 live renderer 中沒有 `█/▓/▒/░` 或連續 `●/○` meter；`●` 在 live HUD 的已定義用途是 Git dirty。Color picker 內出現的 `██` 只是 configurator 色塊樣本，不是 statusline progress bar。[color picker sample](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/src/ui/components/color_picker.rs#L427-L459)

### 3.2 對本專案的具體建議

以下是基於上游觀察與本專案既有 UI 的產品建議，不是聲稱 `best-claude-hud` 已採用這些規則：

1. **dot 固定有間隔**：十個位置以單一 ASCII 空格 join，沒有前導或尾端空格。

   ```text
   ● ● ● ○ ○ ○ ○ ○ ○ ○
   ```

2. **bar 固定連續**：filled `▓` 與 empty `░` 中間也不插入空格。

   ```text
   ▓▓▓░░░░░░░
   ```

3. 同一個 meter builder 應讓 context、5h、7d、Fable 5 共用上述規則；避免四處各自拼接後出現不一致。
4. 測試除了比對完整 dot 範例，也應明確否定 bar 中的 `▓ ▓` 與 `░ ░`，讓「dot 疏、bar 密」成為 regression contract。
5. Dot 模式由 10 columns 增加為 19 columns（10 glyph + 9 spaces）。本專案會依寬度主動切成兩行，因此 visible-length 計算必須包含這 9 個空格；若 terminal 對 emoji/variation selector 寬度判定不一致，可借鏡上游以 terminal column width 而非單純字元數處理寬度。[上游 Unicode width 使用](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/src/core/statusline.rs#L1-L34) [相關 changelog](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/CHANGELOG.md#L16-L23)
6. 若要借用上游 icon 語彙，最小且清楚的對應是 model `🤖`、effort `🧠`、workspace `📁`、Git `🌿`、context `⚡️`、quota/usage `📊`。不建議為 5h、7d、Fable 5 各塞一個 emoji；它們已有文字 label，額外 icon 會顯著增加 statusline 寬度。
7. 布局是否改成「model/effort → workspace/Git → context/limits」屬於產品選擇。上游的證據支持此資訊層級，但本專案目前把 workspace 放前面、超寬時把 usage block 整組移到第二行；若變更順序，應視為獨立 UI 變更，不應與 dot spacing 的小修混在一起。
