# 以 Rust + Node.js 重寫 StatusLine：技術調研

> 調研日期：2026-08-22
> 調研對象：將 `claudeStatusLine.sh`（v1.2.2、953 行 bash）重寫為 Rust + Node.js 專案的技術路線
> 來源政策：本文只引用一手來源 — 官方文件、規格、以及各專案的**實際原始碼**。不引用部落格或二手整理。

---

## 摘要

### 三個關鍵結論

**1. 現行 shell script 有一半的工作是多餘的 — Claude Code 現在直接在 stdin 上提供這些資料。**

最重要的發現是 `effort.level`。現行腳本為了取得 reasoning effort，讀取 `CLAUDE_CODE_EFFORT_LEVEL` 環境變數，否則去解析 `~/.claude/settings.json` 的 `.effortLevel`（`claudeStatusLine.sh:280-296`）。官方文件現在明確記載 stdin payload 內含 `effort.level`，且「Reflects the live session value, including mid-session `/effort` changes」（[statusline 文件](https://code.claude.com/docs/en/statusline)）。腳本目前的作法**讀不到 session 中途用 `/effort` 改過的值** — 它讀的是設定檔的靜態值。改用 stdin 欄位同時消除一次檔案 I/O、一次 `jq` 子行程，並修正一個實質正確性缺陷。

同樣地，`context_window.used_percentage` 已經是預先算好的（官方文件說明它用 input-only 公式：`input_tokens + cache_creation_input_tokens + cache_read_input_tokens`），而 `COLUMNS` / `LINES` 環境變數是官方保證會設定的（需 Claude Code v2.1.153+）— 腳本現有的 `$COLUMNS` 用法有官方背書，可以放心保留。

**2. napi-rs 不適用，原因是結構性的而非效能調校問題。**

這是本次調研中證據最硬的一項。`.node` addon 是**函式庫，不是可執行檔** — 它必須由一個 Node 行程載入。實測（Apple Silicon、Node v26.7.0、丟棄暖機後取平均）：

| 方案 | 每次呼叫耗時 |
|---|---|
| 原生 Rust binary（直接執行） | **1.73–2.03 ms** |
| `/bin/echo`（fork+exec 地板值） | 1.47–1.67 ms |
| `node -e ""`（空的 Node 啟動） | **20.24 ms** |
| napi-rs addon（`require` 真實 `@node-rs/argon2`） | 22.02 ms |
| Node shim 再 spawn 原生 binary | 23.73–28.55 ms |

napi-rs 的 addon 載入本身只多花 ~1.8 ms — **慢的是 Node 本身，不是 napi-rs**。而 napi-rs 無法移除 Node，因為「需要 Node」就是這項技術的定義。

決定性的一手佐證來自 esbuild 自己的安裝程式。[`lib/npm/node-install.ts`](https://github.com/evanw/esbuild/blob/main/lib/npm/node-install.ts) 的 `maybeOptimizePackage()` 註解寫得毫不含糊：

> This package contains a "bin/esbuild" JavaScript file that finds and runs the appropriate binary executable. However, **this means that running the "esbuild" command runs another instance of "node" which is way slower than just running the binary executable directly.** Here we optimize for this by **replacing the JavaScript file with the binary executable at install time.**

實測驗證了這件事確實發生：`npm install esbuild` 之後，`node_modules/esbuild/bin/esbuild` 是 `Mach-O 64-bit executable arm64`，不是 JS。esbuild 因此是 4.14 ms；保留 JS shim 的 Biome 是 28.55 ms — **同樣的架構、同樣的原生 binary，差 7 倍**。

**3. WASM 直接出局 — 不是慢，是做不到。**

[WASI preview1 完整介面](https://github.com/WebAssembly/WASI/blob/wasi-0.1/preview1/witx/wasi_snapshot_preview1.witx)共 46 個函式，**沒有 spawn / exec / fork**（只有針對自己的 `proc_exit` 和 `proc_raise`），所以 WASM 在 Node 的 WASI 下**無法執行 `git`**。網路方面只有 `sock_accept/recv/send/shutdown`，**沒有 `sock_open`、沒有 `sock_connect`、沒有 DNS**，所以無法主動發起 HTTPS 連線。Node 實際使用的 [uvwasi](https://github.com/nodejs/uvwasi/blob/main/README.md) 更少 — 43 個 call，且完全沒實作 `sock_accept`。此外 [`node:wasi`](https://nodejs.org/api/wasi.html) 至今仍標示 Stability 1 - Experimental。

### 架構建議

**採用「原生 Rust binary + 透過 npm 發佈 + `statusLine.command` 直接指向原生 binary」。**

關鍵設計決策是 **postinstall 把 binary 複製到 `~/.claude/<name>/<name>`**，然後在 README 指示使用者設定 `"command": "~/.claude/statusline-rs/statusline-rs"`。這一步同時解決兩件事：render 路徑上完全沒有 Node（省掉 ~20 ms），以及 `settings.json` 裡的路徑與平台無關、可跨機器同步。這正是目前 Rust statusline 生態中最成熟的專案 [CCometixLine](https://github.com/Haleclipse/CCometixLine) 採用的模式。

排序如下：

| 排名 | 方案 | 每次 render 成本 | 結論 |
|---|---|---|---|
| **1** | **Rust binary via npm，`command` 直指原生 binary** | **~2 ms** | **採用。** 有 npm 的發佈便利性，render 時零 Node。 |
| 2 | Rust binary，非 npm 發佈（GitHub Releases / Homebrew） | ~2 ms | 執行特性相同，發佈較麻煩。併入方案 1 一起做。 |
| 3 | npm bin JS wrapper（Node shim spawn binary） | ~24–29 ms | 同一個 binary 慢 6 倍，沒有意義。 |
| 4 | napi-rs `.node` addon | ~22 ms | 結構上必須有 Node。對 CLI 而言是錯的工具。 |
| 5 | WASM | 不適用 | WASI 無法 spawn `git`、無法開 socket。 |
| — | Claude Agent SDK | 不適用 | 與 statusline 無關（見第三節 E）。 |

**關於「Node.js SDK」的釐清：** 如果指的是 `@anthropic-ai/claude-agent-sdk`，它與 statusline **完全無關**，且方向相反 — Agent SDK 是「你的程式去呼叫 Claude」的客戶端，statusline 是「Claude Code 回呼你的程式」。詳見第三節 E。本文假設「Rust + Node.js SDK」指的是「Rust 核心 + 用 npm 生態發佈」。

---

## 一、Claude Code statusline 輸入／輸出契約

一手來源：<https://code.claude.com/docs/en/statusline> 與 <https://code.claude.com/docs/en/settings-reference>

### 1.1 settings.json 設定

`statusLine` 的權威型別定義來自 settings reference（[原始 Markdown](https://code.claude.com/docs/en/settings-reference.md) 的 `### statusLine` 段落，逐字引用）：

> **Type**: object with `type` set to `"command"` and a `command` string, plus optional `padding` as a number of characters, `refreshInterval` as a number of seconds, minimum `1`, and `hideVimModeIndicator` as a Boolean
> **Default**: unset, so no status line

| 欄位 | 型別 | 說明 | 來源 |
|---|---|---|---|
| `type` | `"command"` | 目前僅此一種 | [settings-reference](https://code.claude.com/docs/en/settings-reference) |
| `command` | string | 腳本路徑、**原生可執行檔路徑**，或 inline shell 指令。「The `command` field runs in a shell」 | [statusline](https://code.claude.com/docs/en/statusline) |
| `padding` | number（字元數） | 預設 `0`。「This padding is in addition to the interface's built-in spacing, so it controls relative indentation rather than absolute distance from the terminal edge」 | [statusline](https://code.claude.com/docs/en/statusline) |
| `refreshInterval` | number（**秒**，最小 `1`） | 在事件驅動更新之外，額外每 N 秒重跑一次。適用於顯示時間類資料，或主 session 閒置時背景 subagent 改動了 git 狀態 | [statusline](https://code.claude.com/docs/en/statusline) |
| `hideVimModeIndicator` | boolean | 預設 `false`。腳本自行渲染 `vim.mode` 時設為 `true`，避免重複顯示 | [statusline](https://code.claude.com/docs/en/statusline) |

> **注意一處文件內部不一致：** `code.claude.com/docs/en/settings-reference` 這個頁面透過某些 HTML 轉譯路徑讀取時，會呈現 `padding` 為 string（預設 `"  "`）、`refreshInterval` 單位為毫秒。實際抓取該頁的原始 Markdown（`settings-reference.md`）確認正確定義是 **`padding` 為字元數、`refreshInterval` 為秒且最小值 1**，與 statusline 專頁一致。本文採用後者。

`command` 可以直接是原生可執行檔路徑這點很關鍵 — 這正是「跳過 Node」策略成立的前提。

### 1.2 stdin JSON 完整 schema

官方在 statusline 頁的「Full JSON schema」摺疊區提供完整結構。以下表格逐欄列出每個記載欄位。

| 欄位路徑 | 型別 | 說明 |
|---|---|---|
| `cwd` | string | 目前工作目錄。與 `workspace.current_dir` 值相同 |
| `session_id` | string | Session 唯一識別碼 |
| `session_name` | string | Session 名稱（`--name` 旗標或 `/rename` 設定的自訂名稱，否則為 AI 產生的標題）。**可能不存在** — 預設顯示名稱如 `my-app-3f` 不會填入此欄 |
| `prompt_id` | string (UUID) | 目前處理中的 user prompt UUID，對應 OpenTelemetry 事件的 `prompt.id`。**首次輸入前不存在**。需 v2.1.196+ |
| `transcript_path` | string | 對話 transcript 檔案（`.jsonl`）路徑 |
| `model.id` | string | 模型識別碼，例如 `"claude-opus-5"` |
| `model.display_name` | string | 模型顯示名稱，例如 `"Opus"` |
| `workspace.current_dir` | string | 目前工作目錄。官方建議優先使用此欄而非 `cwd`（與 `project_dir` 一致性） |
| `workspace.project_dir` | string | Claude Code 啟動時的目錄；session 中途改變 cwd 時會與 `cwd` 不同 |
| `workspace.added_dirs` | string[] | 透過 `/add-dir` 或 `--add-dir` 加入的額外目錄。無則為空陣列 |
| `workspace.git_worktree` | string | 目前目錄位於 `git worktree add` 建立的 linked worktree 時的 worktree 名稱。主工作樹中不存在。與只適用 `--worktree` session 的 `worktree.*` 不同，此欄對任何 git worktree 都會填入 |
| `workspace.repo.host` | string | 從 `origin` remote 解析，例如 `"github.com"` |
| `workspace.repo.owner` | string | 例如 `"anthropics"` |
| `workspace.repo.name` | string | 例如 `"claude-code"`。**非 git repo 或無 `origin` remote 時 `repo` 整個不存在** |
| `version` | string | Claude Code 版本，例如 `"2.1.90"` |
| `output_style.name` | string | 目前 output style 名稱 |
| `cost.total_cost_usd` | number | Session 預估成本（USD），**client 端計算**，可能與實際帳單不同。`/clear` 開新 session 後歸零 |
| `cost.total_duration_ms` | number | Session 開始至今的 wall-clock 時間（毫秒） |
| `cost.total_api_duration_ms` | number | 等待 API 回應的總時間（毫秒） |
| `cost.total_lines_added` | number | 新增行數 |
| `cost.total_lines_removed` | number | 刪除行數 |
| `context_window.total_input_tokens` | number | 目前 context window 中的 input token 數（**含 cache 讀寫**）。等於 `input_tokens + cache_creation_input_tokens + cache_read_input_tokens`。首次 API 回應前為 `0` |
| `context_window.total_output_tokens` | number | 最近一次回應的 output token 數。首次 API 回應前為 `0` |
| `context_window.context_window_size` | number | Context window 上限。預設 `200000`，延伸 context 模型為 `1000000` |
| `context_window.used_percentage` | number | **預先算好的**已用百分比。**session 早期可能為 `null`** |
| `context_window.remaining_percentage` | number | 預先算好的剩餘百分比。**可能為 `null`** |
| `context_window.current_usage.input_tokens` | number | 目前 context 中的 input tokens |
| `context_window.current_usage.output_tokens` | number | 產生的 output tokens |
| `context_window.current_usage.cache_creation_input_tokens` | number | 寫入 cache 的 tokens |
| `context_window.current_usage.cache_read_input_tokens` | number | 從 cache 讀取的 tokens。**整個 `current_usage` 在首次 API 呼叫前為 `null`，`/compact` 後到下次 API 呼叫前也是 `null`** |
| `exceeds_200k_tokens` | boolean | 最近一次回應的 total token（input + cache + output）是否超過 200k。**固定門檻，與實際 context window 大小無關** |
| `fast_mode` | boolean | 該 session 是否啟用 fast mode |
| `effort.level` | string | 目前 reasoning effort：`low` / `medium` / `high` / `xhigh` / `max`。**反映 session 即時值，含中途 `/effort` 變更**。Ultracode 不是獨立等級，回報為 `xhigh`。**模型不支援 effort 參數時整個 `effort` 不存在** |
| `thinking.enabled` | boolean | 該 session 是否啟用 extended thinking |
| `rate_limits.five_hour.used_percentage` | number | 5 小時額度已用百分比（0–100） |
| `rate_limits.five_hour.resets_at` | number | 5 小時視窗重置的 **Unix epoch 秒數** |
| `rate_limits.seven_day.used_percentage` | number | 7 日額度已用百分比（0–100） |
| `rate_limits.seven_day.resets_at` | number | 7 日視窗重置的 Unix epoch 秒數。**`rate_limits` 只對 Claude.ai 訂閱者（Pro/Max）在該 session 首次 API 回應後出現，且 `five_hour` 與 `seven_day` 可能各自獨立不存在** |
| `vim.mode` | string | `NORMAL` / `INSERT` / `VISUAL` / `VISUAL LINE`。**僅在 vim mode 啟用時存在** |
| `agent.name` | string | 使用 `--agent` 旗標或設定 agent 時的 agent 名稱。**否則不存在** |
| `pr.number` | number | 目前分支的開啟中 PR 編號。GitLab remote 時為 merge request 編號（需 v2.1.234+） |
| `pr.url` | string | PR / MR 連結 |
| `pr.review_state` | string | `approved` / `pending` / `changes_requested` / `draft`。**即使 `pr` 存在也可能獨立不存在** |
| `pr.kind` | string | GitLab merge request 時為 `mr`；GitHub PR 時不存在（需 v2.1.234+） |
| `worktree.name` | string | 使用中的 worktree 名稱。**僅在 `--worktree` session 中存在** |
| `worktree.path` | string | Worktree 目錄絕對路徑 |
| `worktree.branch` | string | Worktree 的 git 分支名。**hook-based worktree 時不存在** |
| `worktree.original_cwd` | string | 進入 worktree 前所在的目錄 |
| `worktree.original_branch` | string | 進入 worktree 前 checkout 的分支。**hook-based worktree 時不存在** |

官方完整 payload 範例（逐字引用自 [statusline 文件](https://code.claude.com/docs/en/statusline) 的 Full JSON schema）：

```json
{
  "cwd": "/current/working/directory",
  "session_id": "abc123...",
  "session_name": "my-session",
  "prompt_id": "550e8400-e29b-41d4-a716-446655440000",
  "transcript_path": "/path/to/transcript.jsonl",
  "model": { "id": "claude-opus-5", "display_name": "Opus" },
  "workspace": {
    "current_dir": "/current/working/directory",
    "project_dir": "/original/project/directory",
    "added_dirs": [],
    "git_worktree": "feature-xyz",
    "repo": { "host": "github.com", "owner": "anthropics", "name": "claude-code" }
  },
  "version": "2.1.90",
  "output_style": { "name": "default" },
  "cost": {
    "total_cost_usd": 0.01234,
    "total_duration_ms": 45000,
    "total_api_duration_ms": 2300,
    "total_lines_added": 156,
    "total_lines_removed": 23
  },
  "context_window": {
    "total_input_tokens": 15500,
    "total_output_tokens": 1200,
    "context_window_size": 200000,
    "used_percentage": 8,
    "remaining_percentage": 92,
    "current_usage": {
      "input_tokens": 8500,
      "output_tokens": 1200,
      "cache_creation_input_tokens": 5000,
      "cache_read_input_tokens": 2000
    }
  },
  "exceeds_200k_tokens": false,
  "fast_mode": false,
  "effort": { "level": "high" },
  "thinking": { "enabled": true },
  "rate_limits": {
    "five_hour": { "used_percentage": 23.5, "resets_at": 1738425600 },
    "seven_day": { "used_percentage": 41.2, "resets_at": 1738857600 }
  },
  "vim": { "mode": "NORMAL" },
  "agent": { "name": "security-reviewer" },
  "pr": {
    "number": 1234,
    "url": "https://github.com/anthropics/claude-code/pull/1234",
    "review_state": "pending"
  },
  "worktree": {
    "name": "my-feature",
    "path": "/path/to/.claude/worktrees/my-feature",
    "branch": "worktree-my-feature",
    "original_cwd": "/path/to/project",
    "original_branch": "main"
  }
}
```

**`context_window` 語意細節**（[statusline 文件](https://code.claude.com/docs/en/statusline) 的 Context window fields 段）：

- `used_percentage` **只由 input token 計算**：`input_tokens + cache_creation_input_tokens + cache_read_input_tokens`，**不含 `output_tokens`**。
- 官方明說：「If you calculate context percentage manually from `current_usage`, use the same input-only formula to match `used_percentage`.」
- 現行 `claudeStatusLine.sh:265` 算的是 `input_tokens + cache_create + cache_read` — **公式與官方一致**，可放心沿用；但既然官方已提供 `used_percentage`，直接用它更省事也更不會偏離。

### 1.3 未記載但實務存在的欄位

社群 TypeScript 實作 [ccstatusline](https://github.com/sirmalloc/ccstatusline)（官方 statusline 文件在 Tips 段主動點名推薦的專案）的 Zod schema [`src/types/StatusJSON.ts`](https://github.com/sirmalloc/ccstatusline/blob/main/src/types/StatusJSON.ts) 包含兩個**官方文件未記載**的事實，直接讀原始碼確認：

```ts
model: z.union([
    z.string(),
    z.object({ id: z.string().optional(), display_name: z.string().optional() })
]).optional(),
```
**`model` 可能是純字串，而不只是物件。**

```ts
rate_limits: z.object({
    five_hour: RateLimitPeriodSchema.optional(),
    seven_day: RateLimitPeriodSchema.optional(),
    seven_day_sonnet: RateLimitPeriodSchema.nullable().optional(),
    seven_day_opus: RateLimitPeriodSchema.nullable().optional()
}).nullable().optional()
```
**`rate_limits` 存在 per-model 的 7 日視窗**（`seven_day_sonnet`、`seven_day_opus`）。這對本專案有直接影響 — 詳見「待決事項」第 1 點。

此外，整個 schema 是 `z.looseObject`（允許未知欄位），且每個數值欄位都經過 `CoercedNumberSchema` 前處理（字串型數字會被轉成 number）。這暗示 payload 在實務上比文件描述更寬鬆，Rust 端的 serde struct 應該同樣防禦性地設計。

### 1.4 stdout 契約

| 項目 | 規則 | 來源 |
|---|---|---|
| 多行 | **不是只顯示第一行。**「each `echo` or `print` statement displays as a separate row」 | [statusline](https://code.claude.com/docs/en/statusline) |
| ANSI 顏色 | 支援。文件直接示範 `\033[32m` | 同上 |
| OSC 8 超連結 | 支援可點擊連結（iTerm2 / Kitty / WezTerm）。Terminal.app 不支援。可用 `FORCE_HYPERLINK=1` 強制 | 同上 |
| 終端寬度 | **`tput cols` 與語言層級的寬度偵測都讀不到** — Claude Code 捕捉 stdout 而非直接接上終端。必須讀 `COLUMNS` / `LINES` 環境變數，Claude Code 會在執行前設定。需 **v2.1.153+** | 同上 |
| stderr | 不顯示。輸出必須到 stdout | 同上（Troubleshooting） |
| 失敗行為 | 「Scripts that exit with non-zero codes or produce no output cause the status line to go blank」 | 同上 |

### 1.5 執行頻率與取消行為

官方「How status lines work」段落：

- 觸發時機：session 開始（含 resume）、**新的 assistant 訊息抵達**、`/compact` 完成、permission mode 改變、vim mode 切換、`refreshInterval` 計時器到期。
- **Debounce 為 300ms** — 快速變化會合併，腳本在變化停止後跑一次。
- **「If a new update triggers while your script is still running, Claude Code cancels the in-flight script.」**

最後一點對設計影響很大：**慢的 statusline 不只是看起來延遲，它可能被中途 kill 而完全不渲染**。這是「所有網路 I/O 都必須走快取、絕不阻塞 render」這條設計原則的正式依據。現行 script 的 60 秒 OAuth 快取與 2 秒 git 快取方向正確，重寫時應保留並強化。

### 1.6 執行環境約束

| 約束 | 內容 | 來源 |
|---|---|---|
| **執行逾時** | **官方文件未記載任何 statusline command 的執行逾時。**已逐字檢查 statusline 專頁與 settings-reference 原始 Markdown，兩者皆無 timeout 參數或行為描述。實際保護機制是上述的「in-flight cancellation」 | [statusline](https://code.claude.com/docs/en/statusline)、[settings-reference.md](https://code.claude.com/docs/en/settings-reference.md) |
| **Workspace trust** | statusLine 執行 shell 指令，因此與 settings 檔案中的 hooks 適用**同一套 workspace trust 規則**。未接受信任前 statusline 一片空白，`claude --debug` 會記錄 `Status line command skipped: workspace trust not accepted` | [statusline](https://code.claude.com/docs/en/statusline) |
| **企業封鎖** | `disableAllHooks` 在 managed settings 之外設定時，只有 managed settings 的 statusLine 會執行；`allowManagedHooksOnly` 開啟時使用者的自訂 statusline 會**無預警消失** | [settings-reference](https://code.claude.com/docs/en/settings-reference) |
| **環境變數** | 官方明確保證的只有 `COLUMNS`、`LINES`（v2.1.153+）與 `FORCE_HYPERLINK`（使用者自行設定）。其餘繼承自 Claude Code 行程 | [statusline](https://code.claude.com/docs/en/statusline) |
| **工作目錄** | 文件未明確記載 statusline command 的 cwd。**不應假設** — payload 已提供 `cwd` 與 `workspace.project_dir`，一律用它們 | — |
| **Windows** | 有 Git Bash 時走 Git Bash，否則走 PowerShell。**Git Bash 會把未加引號的反斜線當跳脫字元**，所以 `C:\Users\...` 這種路徑會被吃掉分隔符且無明顯錯誤。`command` 字串中的路徑一律用正斜線 | [statusline](https://code.claude.com/docs/en/statusline) |
| **通知共用同一行** | MCP server 錯誤、自動更新等系統通知顯示在同一行的右側；verbose mode 還會加上 token 計數器。**窄終端上這些通知會截斷你的輸出** | [statusline](https://code.claude.com/docs/en/statusline) |

### 1.7 順帶記錄：`subagentStatusLine`

`subagentStatusLine` 是獨立的設定（[settings-reference](https://code.claude.com/docs/en/settings-reference)），型別為 `{ type: "command", command: string }`。它每個 refresh tick 執行一次，stdin 收到含 [base hook 欄位](https://code.claude.com/docs/en/hooks#common-input-fields)、`columns`（可用列寬）與 `tasks` 陣列的單一 JSON 物件。每個 task 有 `id`、`name`、`type`、`status`、`description`、`label`、`startTime`、`model`、`effort`、`contextWindowSize`、`tokenCount`、`tokenSamples`、`cwd`。

輸出格式是每行一個 JSON：`{"id": "<task id>", "content": "<row body>"}`。`content` 原樣渲染（含 ANSI 與 OSC 8）。省略某個 task 的 `id` 保留預設渲染；`content` 為空字串則隱藏該行。

適用同一套 trust / `disableAllHooks` / `allowManagedHooksOnly` 限制。這是重寫時可以順手支援的一個延伸功能（同一個 binary 加一個 `--subagent` 子命令即可）。

---

## 二、Rust 版 Claude Code statusline 生態調查

以下所有專案都經 `gh api` 驗證存在，並直接閱讀原始碼（非 README）。

### 2.0 生態全貌與一個重要的血緣發現

| Repo | Stars | 最後推送 | License | 語言 |
|---|---|---|---|---|
| [Haleclipse/CCometixLine](https://github.com/Haleclipse/CCometixLine) | 3449 | 2026-03-14 | **無 LICENSE 檔**（Cargo.toml 宣稱 MIT） | Rust |
| [GaoSSR/best-claude-hud](https://github.com/GaoSSR/best-claude-hud) | 346 | 2026-08-13 | Apache-2.0 | Rust |
| [Wangnov/claude-code-statusline-pro](https://github.com/Wangnov/claude-code-statusline-pro) | 234 | 2026-08-17 | MIT | Rust |
| [glauberlima/claude-code-statusline](https://github.com/glauberlima/claude-code-statusline) | 49 | 2026-08-19 | MIT | Rust |
| [khoi/cc-statusline-rs](https://github.com/khoi/cc-statusline-rs) | 42 | 2026-07-05 | MIT | Rust |
| [MaurUppi/CCstatus](https://github.com/MaurUppi/CCstatus) | 35 | 2025-09-11 | 無 LICENSE 檔 | Rust |
| [ding113/ccline-packycc](https://github.com/ding113/ccline-packycc) | 30 | 2025-08-28 | MIT | Rust |
| [sotayamashita/claude-code-statusline](https://github.com/sotayamashita/claude-code-statusline) | 7 | 2026-04-17 | MIT | Rust |
| [sirmalloc/ccstatusline](https://github.com/sirmalloc/ccstatusline) | **12508** | 2026-08-17 | MIT | TypeScript |

`ndave92/claude-code-status-line`（搜尋結果中出現過）**實際不存在，`gh api` 回 404**。

**血緣發現：CCometixLine、best-claude-hud、ccline-packycc、CCstatus 實質上是同一份 codebase。** 逐檔比對 CCometixLine 與 best-claude-hud：**51 個 `.rs` 檔中有 27 個位元組完全相同**，包括 `src/utils/credentials.rs`、全部 9 個 theme 檔、以及整個 `ui/components/` 目錄。best-claude-hud 甚至保留了一個名為 `cometix` 的主題（`src/ui/themes/theme_cometix.rs`，README 第 184 行寫著 `best-claude-hud --theme cometix`），**README 中對 CCometixLine 零署名**，並在上游沒有 LICENSE 檔的情況下改授權為 Apache-2.0。

所以「Rust 生態」其實只有約 4 種獨立設計：**CCometixLine 血系**、**statusline-pro**、**sotayamashita**、以及玩具專案。**如果要借用 CCometixLine 血系的程式碼，授權來源鏈是混濁的，需要留意。**

另一個值得注意的脈絡：**最受歡迎的 statusline 是 TypeScript 寫的，star 數是任何 Rust 專案的 36 倍**，而且它每次 render 都付完整的 Node 啟動 + `npx` 解析成本，使用者並不在意。這說明這個生態的採用驅動力是**可設定性與完成度，不是效能天花板**。

### 2.1 CCometixLine（3449 stars）— 最紅，工程品質最弱

**Cargo.toml**（v1.1.2）：serde / serde_json / clap / toml / ratatui 0.30 / crossterm 0.29 / ansi_term / ansi-to-tui / ureq 3.0 / semver / chrono / dirs / regex / tree-sitter。

**完全沒有 `[profile.release]` 段落** — 用 Cargo 預設值：無 LTO、16 codegen units、不 strip、unwinding panic。Release 資產每平台 2.4–3.1 MB（壓縮後）。README 只有行銷語「A high-performance Claude Code statusline tool written in Rust」（README.md:5），**沒有任何 benchmark、hyperfine 或 criterion 數據**。

**stdin 解析**（`src/main.rs:75` 用 `serde_json::from_reader(stdin.lock())`，型別在 `src/config/types.rs`）：

```rust
#[derive(Deserialize)]
pub struct InputData {
    pub model: Model,
    pub workspace: Workspace,
    pub transcript_path: String,
    pub cost: Option<Cost>,
    pub output_style: Option<OutputStyle>,
}
```

`model`、`workspace`、`transcript_path` **是必填非 Option** — payload 缺任一個就讓 `main()` 回 `Err` 且完全不輸出。**沒有 `session_id`、沒有 `context_window`、沒有 `rate_limits`** — 它落後現行 payload 一大截，仍在自己解析 transcript 取得 Claude Code 現在免費奉送的資料。

它做得好的地方是 usage 的跨供應商正規化 — `RawUsage` 有 12 個 optional 欄位同時涵蓋 Anthropic 與 OpenAI 命名，加一個 catch-all：

```rust
#[serde(default)] pub input_tokens: Option<u32>,
#[serde(default)] pub prompt_tokens: Option<u32>,
#[serde(default)] pub cache_creation_input_tokens: Option<u32>,
#[serde(default)] pub cache_read_input_tokens: Option<u32>,
#[serde(flatten, skip_serializing)] pub extra: HashMap<String, serde_json::Value>,
```

正規化為 `NormalizedUsage { ..., calculation_source: String, raw_data_available: Vec<String> }` — 帶著「這個數字是怎麼算出來的」的稽核軌跡。**這個模式值得抄。**

**渲染**：手刻原始 ANSI，不用任何 crate（`src/core/statusline.rs`）：

```rust
Some(AnsiColor::Rgb { r, g, b }) => format!("\x1b[38;2;{};{};{}m{}\x1b[0m", r, g, b, text),
```

**重大缺陷**：寬度函式數的是 char 不是欄寬 —

```rust
fn visible_width(text: &str) -> usize {
    // ...逐字元剝除 ESC 序列...
    visible.chars().count()
}
```

沒有 `unicode-width` 依賴。CJK、emoji、Nerd Font glyph 全部量錯。而且該函式只在 TUI 預覽路徑被呼叫 — **實際 statusline 輸出完全不做寬度偵測、不做截斷**，直接印出讓終端自己 wrap。

**Transcript 讀取**（`src/core/segments/context_window.rs`）— 這是效能災難：

```rust
fn try_parse_transcript_file(path: &Path) -> Option<u32> {
    let file = fs::File::open(path).ok()?;
    let reader = BufReader::new(file);
    let lines: Vec<String> = reader.lines().collect::<Result<Vec<_>, _>>().unwrap_or_default();
    for line in lines.iter().rev() {
        if let Ok(entry) = serde_json::from_str::<TranscriptEntry>(line) {
            if entry.r#type.as_deref() == Some("assistant") { /* message.usage */ }
        }
    }
}
```

**每次呼叫都把整份 `.jsonl` 讀進 `Vec<String>`**，再往回走。長 session 就是每次 render 配置數十 MB。沒有 offset 快取、沒有 tail seek、沒有 mtime 檢查。更糟的是兩條 fallback 路徑會讀**更多**檔案：最後一行是 `type: "summary"` 時會掃描專案目錄下**每一個 `.jsonl`** 解析 `leafUuid`；transcript 不存在時會 `read_dir` 整個專案、按 mtime 排序、逐一完整解析直到找到 usage。**這是最不該抄的東西。**

**外部資料源**（三個都做）：
- **macOS Keychain**（`src/utils/credentials.rs`）：`Command::new("security").args(["find-generic-password", "-a", &user, "-w", "-s", "Claude Code-credentials"])`，解析 `claudeAiOauth.accessToken`，fallback 到 `$CLAUDE_CONFIG_DIR/.credentials.json` 再到 `~/.claude/.credentials.json`。**與現行 `claudeStatusLine.sh:477-508` 的作法幾乎一致。**
- **Anthropic OAuth usage endpoint**（`src/core/segments/usage.rs`）：`GET {base}/api/oauth/usage`，帶 `Authorization: Bearer`、`anthropic-beta: oauth-2025-04-20`，以及一個透過**執行 `npm view @anthropic-ai/claude-code version`** 組出來的 User-Agent — **在 statusline render 中對 npm registry 發網路請求**。快取到 `~/.claude/ccline/.api_usage_cache.json`，預設 TTL 300 秒，失敗時回退 stale cache（這點好）。現行 shell script 走的是同一個 endpoint 與同一組 header（`claudeStatusLine.sh:549-557`），但 User-Agent 是硬編的 `claude-code/2.1.34`，比 CCometixLine 的作法便宜且安全。
- **Git**：`std::process::Command` 直接呼叫 `git` 二進位，不用 git2/gix，且**每次呼叫都正確加上 `--no-optional-locks`**（`src/core/segments/git.rs`）。但它每次 render 開**五個** git 行程：`rev-parse --git-dir`、`branch --show-current`、`status --porcelain`、`rev-list --count @{u}..HEAD`、`rev-list --count HEAD..@{u}`。相較之下現行 shell script 用**單一** `status --porcelain=v2 --branch` 就同時拿到分支與狀態（`claudeStatusLine.sh:365-367`）— **現行腳本在這一點上比 CCometixLine 更好，重寫時務必保留。**

**發佈方式** — npm 優先，這部分最值得抄。`npm/main/package.json` 用 per-platform `optionalDependencies`（7 個套件，**含獨立的 musl 變體**）：

```json
"bin": { "ccline": "./bin/ccline.js" },
"scripts": { "postinstall": "node scripts/postinstall.js" },
"optionalDependencies": {
  "@cometix/ccline-darwin-x64": "0.0.0", "@cometix/ccline-darwin-arm64": "0.0.0",
  "@cometix/ccline-linux-x64": "0.0.0", "@cometix/ccline-linux-x64-musl": "0.0.0",
  "@cometix/ccline-linux-arm64": "0.0.0", "@cometix/ccline-linux-arm64-musl": "0.0.0",
  "@cometix/ccline-win32-x64": "0.0.0"
}
```

平台套件是 `{ "files": ["ccline"], "os": ["darwin"], "cpu": ["arm64"] }` — **由 npm 自己的 resolver 選擇，沒有 postinstall 下載**。

而 `postinstall.js` 做了關鍵的一步：把解析出的 binary **複製／硬連結到 `~/.claude/ccline/ccline`**，讓使用者可以在 `settings.json` 指向一個穩定路徑而非 node_modules 路徑；`bin/ccline.js` 也**優先檢查該路徑**再 fallback 回 node_modules（讓自我更新過的 binary 勝出）。它還有真正的 libc 偵測（跑 `ldd --version`、解析 glibc 主次版本、glibc < 2.35 時選 musl、偵測失敗預設 musl）。Release workflow 用 **cargo-zigbuild + Zig 當交叉連結器**建置 7 個 target（`.github/workflows/release.yml:59-83`），先發平台套件、等 registry、再發主套件。

**另一個值得抄的巧思** — `src/config/models.rs` 用 regex 產生模型家族，而不是硬編 `claude-opus-4-6-20250901`：

```rust
let pattern = format!(
    r"(?:(?P<pre_major>\d{{1,2}})(?:-(?P<pre_minor>\d{{1,2}}))?-{kw}|{kw}-(?P<post_major>\d{{1,2}})(?:-(?P<post_minor>\d{{1,2}}))?)(?:-\d{{3,}}|-[a-z]|\[|$)",
    kw = keyword);
```

尾端的邊界交替 `-\d{3,}|-[a-z]|\[|$` 是在沒有 lookahead 的情況下（`regex` crate 不支援）阻止日期後綴被當成 minor version 的手法，透過 `OnceLock` 只編譯一次。另有獨立的 `ContextModifier` 處理 `[1m]` → 1M window — **正好對應你自己的 model ID `claude-opus-5[1m]` 這種情況**。

**不建議碰的**：`src/utils/claude_code_patcher.rs`（844 行）用 tree-sitter + tree-sitter-javascript **解析 Claude Code 自己的 `cli.js` AST** 並 patch 掉「Context low」警告。每次 Claude Code 更新就會壞。

### 2.2 best-claude-hud（346 stars）— 修好了大部分缺陷的 fork

（本 repo 的 `docs/research/best-claude-hud-layout-and-icons.md` 已對其 UI 詞彙做過分析；這裡補的是工程面。）

架構同上，但修掉了 CCometixLine 的多數實質缺陷。`Cargo.toml`（v0.1.11）加入 `sysinfo`、**`unicode-width = "0.2"`**，關鍵是：

```toml
[profile.release]
opt-level = "z"
strip = true
lto = true
codegen-units = 1
panic = "abort"
```

對重寫有意義的差異：

- **讀現代 payload。** `InputData` 加了 `session_id`、`rate_limits`、`context_window`、`effort` — **全部是 `Option` + `#[serde(default)]`**。`Model` 有**自訂 `Deserialize` 實作，接受純字串或物件**（探測 `id` / `model` / `name`）。`RateLimitWindow` 用 `#[serde(default, alias = "usedPercentage", alias = "used_percent", alias = "utilization")]`。
- **優先用 stdin，transcript 只當 fallback。** Context 先讀 stdin 的 `context_window`；rate_limits 同理 — **stdin 沒資料時才發網路請求**。這正是本專案該採的策略。
- **真正的寬度處理。** `visible_width()` 剝除 ANSI 後呼叫 `UnicodeWidthStr::width()`，並有測試斷言 `"🤖 Kimi K2.7 | 🧠 max"` 是 21 欄。
- **以 `project_dir` 為錨。** `Workspace::project_directory()` 優先用 `project_dir` 而非 `current_dir`，所以 skill 或 subagent `cd` 走時各 segment 不會閃動。小但真實的打磨。
- **Transcript 品質修正。** 優先選有 `stop_reason` 的 assistant entry（完成的回合）而非串流快照；用 `(tokens > 0).then_some(tokens)` 丟棄使用者中斷後寫入的全零 usage 佔位。

**最有價值的獨創是 `src/core/effort.rs`** — 一個增量、抗竄改的 transcript 快取：

```rust
struct EffortCache {
    cache_version: u8, transcript_path: String, started_at: u64,
    processed_bytes: u64, selection: Option<EffortSelection>,
    pending_prompt_id: Option<String>,
    prefix_checkpoint: Vec<u8>, checkpoint: Vec<u8>,
    processed_hash: u64,
    created_nanos: Option<u64>, modified_nanos: Option<u64>, changed_nanos: Option<u64>,
}
```

它 `seek(SeekFrom::Start(cache.processed_bytes))` 只讀新增的尾段，用 file creation time + mtime + **ctime**（`MetadataExt::ctime`）加上 FNV-1a 滾動雜湊與 64 bytes 頭／尾 checkpoint 驗證 — 這同時抓得到截斷**和**「同長度內容替換」（單靠 mtime 抓不到）。透過 temp file + rename 原子寫入 `dirs::cache_dir()/best-claude-hud/effort-{session_id}-{started_at}.json`。

它還**把 transcript 內容當成敵意輸入**：`/effort` 指令輸出只有在綁定到時間戳 ≥ session 開始的對應 command entry 的 `pending_prompt_id` 時才採計，單元測試名稱是 `arbitrary_stdout_cannot_spoof_ultracode` 和 `stdout_with_embedded_effort_tag_cannot_create_fake_ultracode_success`。**這個威脅模型是正確的** — transcript 裡的工具輸出是不可信輸入。

> **不過對本專案來說，這整套 effort 快取機制是可以直接省掉的** — Claude Code 現在在 stdin 的 `effort.level` 直接給了即時值（見 1.2 節）。best-claude-hud 的複雜度是為了在該欄位存在之前逆向工程出同樣的資訊。**這是重寫時應該刻意不抄的東西。**

**發佈**：比上游乾淨 — 4 個平台套件走 `optionalDependencies`，`lib/resolve-binary.js` 把 `${platform}-${arch}` 映射到 vendored 路徑，**完全沒有 postinstall script**。Release workflow 有 `validate-version` job 交叉檢查 tag / Cargo.toml / package.json 才建置。但只有 4 個 target — **沒有 Linux ARM64**，相對上游的 7 個是退步。另有 `--setup` 旗標（`src/utils/claude_settings.rs`）把 `statusLine` 合併進 `~/.claude/settings.json`，備份成帶時間戳的 `.bak` 並 chmod `0o600`。

### 2.3 claude-code-statusline-pro（234 stars）— transcript 讀得最好的一個

不在原始清單中，透過 `gh search repos` 找到。20,969 LOC / 49 個檔案。**這是唯一把 transcript 問題真正解決的專案** — `src/storage/manager.rs::read_tokens_from_transcript`：

```rust
let mut offset = snapshot.transcript_state.processed_offset;
let needs_reset = snapshot.transcript_state.transcript_path.as_deref() != Some(transcript_path)
    || offset > file_len;
let mut file = File::open(path)?;
file.seek(SeekFrom::Start(offset))?;
let mut reader = BufReader::new(file);
```

然後在迴圈中 `read_line` 並追蹤 `current_offset`，把 `TokenHistory` + byte offset 原子地持久化成 session snapshot。**重複呼叫只讀新增的位元組。** 它也處理 `isCompactSummary` 記錄。README：「Incremental transcript parsing: seeks to the last processed offset and persists snapshots atomically so large `.jsonl` logs no longer stall refreshes.」

其他值得注意的選擇：

- **`git2` 搭配 vendored libgit2 + vendored OpenSSL** — `Repository::discover`、`StatusOptions`、`DescribeOptions`（`src/git/service.rs`），不開行程。代價是 binary 大很多且多一個 C build 依賴。**這是唯一一個用 git2 的專案；其餘全部 shell out。**
- **stdin struct 是所有專案中最防禦性的**（`src/core/input.rs`）：`#[serde(rename_all = "snake_case")]` + 每欄位的 camelCase `alias` + 每欄位 `#[serde(default, skip_serializing_if = "Option::is_none")]` + `#[serde(flatten)] pub extra: Value`。**stdin 為空時回 `InputData::default()` 而非 error。**
- **`[profile.release]` 缺席**；只有 `[profile.dist] inherits = "release", lto = "thin"` 給 cargo-dist 用。
- **和 CCometixLine 一樣的寬度 bug**（`src/core/multiline.rs`）：

  ```rust
  fn truncate_to_width(text: &str, max_width: usize) -> String {
      if text.chars().count() <= max_width { return text.to_string(); }
      text.chars().take(max_width).collect()
  }
  ```

  `unicode-width` 只透過 ratatui 間接出現在 `Cargo.lock`，crate 本身從未 import。
- **發佈最完整**：cargo-dist（`dist-workspace.toml`）建置 6 個 target（含 `aarch64-pc-windows-msvc` 與 musl），產生 shell + powershell + **Homebrew** 安裝器（tap 在 `Wangnov/homebrew-tap`），**外加** npm per-platform `optionalDependencies`，短名 `ccsp` → `npx ccsp@latest`。兩套並行。
- 描述宣稱「10x performance」，但沒有任何 ms / MB 數字；`criterion` 是 dev-dep 但沒有結果表。

### 2.4 CCstatus（35 stars）— 已停更，但有兩個點子值得偷

15,427 行 src / 10,997 行 test。最後推送 2025-09，停更約一年，且早於 `context_window` / `rate_limits` 出現在 stdin，所以它的 `StatuslineInput` **沒有 `#[serde(default)]` 且欄位必填** — 拿今天的 payload 會解析失敗。

```toml
[profile.release]
lto = "thin"
codegen-units = 1
opt-level = "s"
panic = "abort"
strip = "symbols"
```

README 宣稱 **啟動時間 < 50ms、記憶體 < 10MB、binary 3.1MB**（static），並附大小表（預設 4.1MB、network-only 3MB、`timings-curl-static` 7MB）。**repo 內沒有任何 benchmark 佐證這些數字。**

值得偷的：

1. **帶環境變數預算上限的 tail-seek JSONL 讀取**（`src/core/network/jsonl_monitor.rs`）：

   ```rust
   let tail_kb = std::env::var("CCSTATUS_JSONL_TAIL_KB").unwrap_or_else(|_| "64".to_string())
       .parse::<u64>().unwrap_or(64).clamp(1, 10240);
   let tail_bytes = tail_kb * 1024;
   let file_len = file.metadata().await?.len();
   if file_len <= tail_bytes { /* 整份讀 */ }
   let seek_pos = file_len - tail_bytes;
   file.seek(SeekFrom::Start(seek_pos)).await?;
   if let Some(first_newline) = content.find('\n') { Ok(content[first_newline + 1..].to_string()) }
   ```

   記憶體有界，且丟棄不完整的第一行。（諷刺的是它自己的 `src/core/segments/usage.rs` 仍在整份讀 — 這個 codebase 內部不一致。）

2. **由 stdin 觸發的探測視窗，不開背景執行緒** — COLD（session 開始）、RED（錯誤驅動，10 秒節奏）、GREEN（基準線，300 秒）。每次呼叫至多一個 HTTP 探測，依優先序執行。**這是讓一個「每次只活 ~10ms」的行程表現出週期性行為的優雅解法**，對本專案的 OAuth usage 更新頗有參考價值。

3. 原子狀態寫入（temp + `tokio::fs::rename`）與 **`fs2::FileExt::try_lock_exclusive()` 對 sibling `.lock`** 做跨並行呼叫的 log rotation。（現行 shell script 用 `mkdir` 當 lock，`claudeStatusLine.sh:358`、`543` — POSIX 上同樣是原子的，且不需依賴。）

4. 所有專案中最完整的憑證探索鏈：env（`ANTHROPIC_BASE_URL` / `ANTHROPIC_AUTH_TOKEN` / `ANTHROPIC_API_KEY`）→ macOS Keychain OAuth → shell rc 檔 regex 解析 → `~/.claude/settings.json` + `.claude/settings.local.json`。

> **使用前須知**：`src/core/network/oauth_masquerade.rs`（597 行）把探測請求塑造成 `api.anthropic.com/v1/messages` 並偽裝成第一方 CLI — `user-agent: "claude-cli/1.0.103 (external, cli)"`、`x-stainless-*` SDK 指紋 header，以及字面的 `"You are Claude Code, Anthropic's official CLI for Claude."` 當 system prompt。聰明，但踩在 ToS 灰色地帶，**不要抄**。

它的 `visible_width()` 也是數 char 的版本，全 repo 沒有 `unicode-width`。

### 2.5 sotayamashita/claude-code-statusline（7 stars）— star 最低，工程紀律最高

Cargo **workspace**，`edition = "2024"`，三個 crate（`-core` 函式庫 / `-cli` / `test-support`），5,606 LOC。刻意 Starship 導向 — README 開頭解釋它存在是因為「Starship... does not expose a stable, supported Rust library API」。

依賴極簡且**用 feature 分閘**：`dirs`、`serde`、`serde_json`、`toml`、`tracing`、`thiserror`，`git2` 在 `feature = "git"` 之後，`rayon` 在 `parallel` 之後（自述「not used yet」）。`default = []` — 基礎 binary 幾乎不拉任何東西。

它的 stdin 型別是唯一**引用規格出處**的（`crates/claude-code-statusline-core/src/types/claude.rs`）：

```rust
//! Reference: <https://docs.anthropic.com/en/docs/claude-code/statusline#json-input-structure>
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ClaudeInput {
    #[serde(skip_serializing_if = "Option::is_none")] pub hook_event_name: Option<String>,
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")] pub transcript_path: Option<String>,
    pub cwd: String,
    pub model: ModelInfo,
    #[serde(skip_serializing_if = "Option::is_none")] pub workspace: Option<WorkspaceInfo>,
    #[serde(skip_serializing_if = "Option::is_none")] pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub output_style: Option<OutputStyle>,
}
```

（同樣過時 — 無 `context_window` / `rate_limits` / `cost`。）它**完全不讀 transcript**，也沒有 token / context segment。

兩個別人都沒有的東西：

**criterion benchmark 接進 CI gate。** `crates/claude-code-statusline-core/benches/engine_bench.rs` bench `engine.render(&input)`，`Makefile:27-29`：

```make
bench-check: bench
	@python3 scripts/bench_check.py --name engine_render_default --threshold-ms 50
```

由 `.github/workflows/ci.yml:51` 執行，job 名稱就叫「Bench threshold gate」— "Run bench check (mean < 50ms)"。**這是整個 Rust 生態中唯一一個被實際量測的效能數字**；其他每個 repo 的宣稱都是行銷文案。（注意它 bench 的是 render，不含 process 啟動。）

**每模組獨立逾時。** `crates/claude-code-statusline-core/src/timeout.rs` 在 spawn 的執行緒上跑每個模組並用 mpsc channel 溝通，逾時回 `Ok(None)` 並捕捉 panic，所以一個慢模組（巨大 repo 的 git status、NFS 掛載）只會降級該 segment 而不會拖垮整條 statusline。**考量到 1.5 節的 in-flight cancellation 行為，這是正確的失敗模式，而且只有這個專案做了。** 現行 shell script 用 `timeout`/`gtimeout` 包裝達到類似效果（`claudeStatusLine.sh:132-147`），但在沒有這兩個指令的系統上會無保護執行 — Rust 版可以用 thread + channel 無條件保證這件事，**這是重寫的一個明確加分項**。

### 2.6 ccstatusline（12508 stars，TypeScript）— 發佈方式對照組

**不編譯、不做 per-platform 二進位 — 就是一個 bundle 過的 JS 檔。** `package.json`：

```json
"name": "ccstatusline", "version": "2.2.27",
"main": "./dist/ccstatusline.js", "type": "module",
"bin": { "ccstatusline": "./dist/ccstatusline.js" },
"files": ["dist/"],
"scripts": {
  "build": "rm -rf dist/* ; bun build src/ccstatusline.ts --target=node --outfile=dist/ccstatusline.js --target-version=14",
  "prepublishOnly": "bun run build"
},
"engines": { "node": ">=14.0.0" }
```

無 `optionalDependencies`、無二進位、無 postinstall。使用者的設定是 `"command": "npx -y ccstatusline@latest"`。**所以最受歡迎的 statusline 每次 render 都付完整 Node 啟動 + `npx` 解析，而且沒人抱怨。**

它有三個實作細節值得抄：

1. **`compact_boundary` 重置與 main-chain 過濾**（`src/utils/jsonl-metrics.ts`）：透過 `stop_reason` 去重串流片段，並在 `compact_boundary` system 記錄處重置，讓壓縮前的回合不會混入計數；同時過濾掉 sidechain（subagent 回合不該計入你的 context）。

2. **寬度處理是本領域的參考實作**（`src/utils/ansi.ts`）：建立在 `string-width` 套件上，做 grapheme cluster 分段（regional indicator、ZWJ 序列、variation selector、combining mark），並在截斷點**剝除並重新套用** SGR 與 OSC-8 超連結狀態（`truncateStyledText`）。**所有 Rust 專案在這一點上都遠遠落後。** 現行 shell script 用 jq `explode` 自建的寬度計算（`claudeStatusLine.sh:853-884`）處理了 zero-width 與 East Asian wide 範圍，實際上比多數 Rust 專案還準確。

3. **祖先 PID 尋找 TTY**（`src/utils/terminal.ts`）：沿著最多 8 層祖先 PID 用 `ps` 往上找，再試 `stty -F` / `stty -f`，因為 **Claude Code ≥ 2.1.139 spawn statusline 時不帶 controlling terminal**，並提供 `CCSTATUSLINE_WIDTH` 環境變數覆寫。**不過對本專案這是多餘的** — 官方現在保證設定 `COLUMNS`（v2.1.153+，見 1.4 節），直接讀環境變數即可。

### 2.7 邊緣專案

- **khoi/cc-statusline-rs**（42★，378 LOC）：完全沒有型別 struct，用 untyped `serde_json::Value` + `.get().and_then()` 鏈。`reqwest` 是**宣告了但完全沒用的依賴**。它的 `get_session_duration()` transcript 讀取器是**死碼，從未被呼叫**。無 `[profile.release]`。其 `CLAUDE.md` 描述了 PR 狀態、worktree 偵測、`.git/statusbar/` 快取 — **這些程式碼裡全都不存在**。可用的點子：`fish_shorten_path` 與 10 格 `█`/`░` context bar。
- **ding113/ccline-packycc**（30★）：CCometixLine v1.0.4 給 PackyCode 代理用的 fork。唯一改進是**把重依賴 feature 化**（`default = ["tui","self-update","quota","dirs"]`，ratatui / crossterm / ureq / chrono 全 `optional = true`），精簡建置可丟掉 TUI — **這個模式值得抄**。
- **glauberlima/claude-code-statusline**（49★，MIT，活躍）：未深入調查，若要再挖是下一個候選。
- **Darkwing4/statusline-rs-cc**（4★）描述宣稱「~3 ms per invocation」— 除了 CCstatus 的 README 之外，唯一有提到冷啟動數字的專案，未經驗證。

透過 `gh search repos --language rust` 還能找到 Fyko/claudehud（6★）、GregoryHo/cc-pulseline（3★）、Shallow-dusty/horologium（2★）、AzurIce/cc-statusline（2★），以及約 25 個 0–1★ 的專案。長尾很大，但幾乎都是單次提交後棄置。

### 2.8 綜合：該抄什麼、該避開什麼

**四個以上獨立 codebase 驗證過的共識：**
- **shell out 到 `git`，並加 `--no-optional-locks`。** 只有 statusline-pro 用 `git2`，代價是 vendored libgit2 + vendored OpenSSL 的建置負擔。
- **npm `optionalDependencies` + per-platform 二進位套件，不做 postinstall 下載。** 所有認真的專案都收斂到這裡。
- **直接寫原始 ANSI escape 字串。** 熱路徑上沒有人用 `colored` / `anstyle` / `owo-colors`；出現的 ansi crate 全是給 TUI 設定編輯器用的。

**所有專案都做錯、而重寫不該重蹈的五件事：**

1. **整份讀 transcript。** CCometixLine、best-claude-hud 的 context segment、CCstatus 的 usage segment、ccstatusline 全都每次 render 吞下整份 `.jsonl`。只有 statusline-pro（持久化 byte offset）、CCstatus 的 `jsonl_monitor`（tail seek）、best-claude-hud 的 `effort.rs`（offset + hash 驗證）做對了 — 而後兩者只在自己兩個讀取器的其中一個做對。
2. **用 char count 當寬度。** CCometixLine、CCstatus、statusline-pro、cc-statusline-rs 全用 `.chars().count()`。只有 best-claude-hud 引入 `unicode-width`。只有 ccstatusline（TS）做 grapheme cluster + 保留 ANSI 狀態的截斷。
3. **缺 `[profile.release]`。** CCometixLine、cc-statusline-rs、statusline-pro 都沒有。best-claude-hud（`opt-level="z"`, lto, cgu=1, strip, panic=abort）與 CCstatus（`lto="thin"`, `opt-level="s"`）是模板。
4. **stdin struct 過時。** 只有 best-claude-hud 從 stdin 讀 `context_window` 與 `rate_limits`；其他人都在解析 transcript 或發網路請求，去取得 Claude Code 現在免費給的資料。**每個欄位都應該是 `Option` + `#[serde(default)]`，`model` 接受 string-or-object，加 `#[serde(flatten)] extra`，並把 transcript 解析當 fallback。**
5. **沒有量測過的效能數字。** 每個 README 都寫「high-performance」；只有 sotayamashita 有數字，而且是 render 的 CI gate（`< 50ms mean`），不含啟動。**一個公布真實 `hyperfine` 冷啟動數字的重寫，光憑證據就能做出差異化。**

**最值得單獨挑出來抄的點子：** best-claude-hud 的 offset + ctime + FNV 雜湊增量快取，以及把 transcript 內容當敵意輸入的 fail-closed 態度；CCometixLine 的 regex 模型家族 + `OnceLock` + 獨立的 `[1m]` context modifier，以及 normalized usage 上的 `calculation_source` 稽核軌跡；CCstatus 的 `CCSTATUS_JSONL_TAIL_KB` 有界尾讀與 stdin 觸發的 COLD/RED/GREEN 探測視窗；sotayamashita 的每模組 timeout wrapper 與 CI bench gate；ccstatusline 的 `compact_boundary` 重置與 main-chain 過濾；statusline-pro 的 cargo-dist → shell+powershell+Homebrew 安裝器與 npm 並行。

---

## 三、Rust + Node.js 整合方案比較

### 3.1 延遲實測（本節是全文最吃重的證據）

環境：Apple Silicon、macOS 25.5.0（Darwin）、**Node v26.7.0**、page cache 已暖、丟棄 3 次暖機、取 wall-clock 平均。**這些是本次調研的實測值，不是官方數字** — 因為 Node 官方並未發布絕對啟動延遲數據（見下）。

| 編號 | 內容 | ms/次 | n |
|---|---|---|---|
| — | `/bin/echo`（純 fork+exec 地板值） | **1.47–1.67** | 50 |
| **D** | **原生 Rust binary**（stdin→stdout，`opt-level="z"` + LTO + strip） | **1.73–2.03** | 100 |
| **B** | esbuild 走 `node_modules/.bin/esbuild`（已最佳化） | **4.14** | 30 |
| **B** | Biome 原生 binary 直接執行 | **4.91** | 20 |
| — | `node -e ""`（什麼都不做的 Node 啟動） | **20.24** | 100 |
| **A** | `node` + 原始 `process.dlopen()` 載入 `.node` addon | **20.94** | 50 |
| **A** | `node` + `require()` 真實 napi-rs addon（`@node-rs/argon2`） | **22.02** | 50 |
| **B-shim** | `node` 用 `execFileSync` spawn Rust binary | **23.73** | 50 |
| **B-shim** | Biome 走 npm JS bin shim（node spawn 原生） | **28.55** | 20 |

兩個事實跳出來：

1. **Node 的 bootstrap 是 ~20 ms，且不可壓縮。** `.node` addon 只在其上多加 ~0.7–1.8 ms（20.24 → 20.94/22.02）。**napi-rs 不慢，是 Node 慢。** Addon 幾乎免費，載它的 runtime 不是。
2. **「Node shim 再 spawn 原生 binary」是兩邊的缺點兼得**：同一個 Biome binary，28.55 ms vs 直接執行的 4.91 ms — **慢 5.8 倍，~24 ms 純浪費**。

也測過最明顯的緩解手段：[`NODE_COMPILE_CACHE`](https://nodejs.org/api/module.html#module-compile-cache) **完全沒差**（28.24 ms vs 28.55 ms，在雜訊範圍內）。這符合預期 — 它快取的是**使用者 JS 的編譯**，而這裡的成本是 Node 自身的 bootstrap。

**官方佐證：** Node.js 不公布絕對啟動數字，但它維護的 startup benchmark 證實這是被追蹤的成本：[`benchmark/misc/startup-core.js`](https://github.com/nodejs/node/blob/main/benchmark/misc/startup-core.js) 以 `process` 與 `worker` 兩種模式 spawn `benchmark/fixtures/empty.mjs`；[`benchmark/misc/startup-cli-version.js`](https://github.com/nodejs/node/blob/main/benchmark/misc/startup-cli-version.js) 用 `--version` 測 CLI 工具，理由是「the startup cost is still dominated by a more indispensible part of the CLI」。

**決定性的一手來源是 esbuild 自己的安裝程式**（已於摘要引用，此處重述以便對照）：[`lib/npm/node-install.ts`](https://github.com/evanw/esbuild/blob/main/lib/npm/node-install.ts) 的 `maybeOptimizePackage()` 註解說明它在安裝時用二進位執行檔**取代**自己的 JS shim。實測驗證：

```
node_modules/.bin/esbuild -> ../esbuild/bin/esbuild
node_modules/esbuild/bin/esbuild: Mach-O 64-bit executable arm64   ← 不是 JS
```

esbuild 用 `fs.linkSync` → `fs.renameSync`（原子）把平台 binary 硬連結蓋過 JS shim。所以是 4.14 ms。保留 JS shim 的 Biome 是 28.55 ms。**同樣架構，一個最佳化，差 7 倍。**

esbuild 記載的跳過此最佳化的例外：Windows（binary 必須是 `esbuild.exe`）、Yarn（[berry#882](https://github.com/yarnpkg/berry/issues/882) 加冪等性考量）、WASM（[esbuild#4209](https://github.com/evanw/esbuild/issues/4209)）、以及 `--ignore-scripts`。

### 3.2 方案 A — napi-rs 原生 addon

**運作原理**：Node-API（ABI 穩定的 C API）。[napi.rs getting-started](https://napi.rs/docs/introduction/getting-started)：「Node-API makes a native binary ABI-compatible with later Node.js releases that provide the Node-API level it was compiled against.」在 Rust 標註 `#[napi]`，建置產出 `<binaryName>.<platform-arch-abi>.node` 加上 JS loader 與 `.d.ts`。

**工具鏈（`@napi-rs/cli`）**：
- [`napi build`](https://napi.rs/docs/cli/build) — `cargo build --target <triple>` 的包裝。`--platform` 把 target triple 附加到檔名，`--release` 最佳化，`--target` 選 triple。產出 `.node`、在平台變體間切換的 `index.js` loader、以及 `index.d.ts`。
- package.json 的 `napi` 欄位宣告 `binaryName` 與 `targets` — 於 [package-template 的 package.json](https://github.com/napi-rs/package-template/blob/main/package.json) 與實務套件 [`@node-rs/argon2`](https://unpkg.com/@node-rs/argon2@2.1.0/package.json) 確認。
- [發佈流程](https://napi.rs/docs/cli/artifacts)：`napi create-npm-dirs` 在 `npm/` 下產生 per-target 目錄；`napi artifacts`「recursively finds built `.node` and `.wasm` files, validates their binary names and target suffixes, and copies them into the matching per-platform npm packages」；`napi prepublish` 版本化並發佈。

**產生的 per-platform 套件**（真實範例，取自 npm 上的 [`@node-rs/argon2-darwin-arm64`](https://unpkg.com/@node-rs/argon2-darwin-arm64@2.1.0/package.json)）：

```json
{ "name": "@node-rs/argon2-darwin-arm64", "version": "2.1.0",
  "cpu": ["arm64"], "os": ["darwin"],
  "main": "argon2.darwin-arm64.node",
  "files": ["argon2.darwin-arm64.node"] }
```

母套件把全部 13 個列為 `optionalDependencies`。產生的 `index.js`（29KB）是相當實質的 runtime dispatcher — 用三種方式偵測 musl（`readFileSync('/usr/bin/ldd')` → `process.report.getReport()` → `execSync('ldd --version')`），先嘗試本地 `require('./argon2.darwin-arm64.node')` 再 fallback 到 `require('@node-rs/argon2-darwin-arm64')`，並交叉檢查 binding 套件的版本。

**CI 樣板**：[`package-template/.github/workflows/CI.yml`](https://github.com/napi-rs/package-template/blob/main/.github/workflows/CI.yml) 品質很高，涵蓋 **14 個 target**：`x86_64/aarch64/i686-pc-windows-msvc`、`x86_64/aarch64-apple-darwin`、`x86_64/aarch64-unknown-linux-gnu`、`x86_64/aarch64-unknown-linux-musl`、`armv7-unknown-linux-gnueabihf`、`aarch64-linux-android`、`armv7-linux-androideabi`、`x86_64-unknown-freebsd`（透過 `cross-platform-actions` VM）、`wasm32-wasip1-threads`。Linux 交叉編譯用 `--use-napi-cross`，musl 用 zig / `cargo-zigbuild`。在 Node 22/24/26 上測試（含 Docker+QEMU 測 arm）。發佈時設 `npm config set provenance true`，並以 commit message 符合 semver pattern 為閘門。

**適用性結論：不適用。** `.node` addon **是函式庫，無法被執行** — 它需要一個宿主 Node 行程。實測代價：**22.02 ms vs 原生 binary 的 1.73 ms，12.7 倍**。Addon 載入本身幾乎免費（比裸 Node 多 ~1.8 ms）；**問題整個在於那個 Node 行程**，而 napi-rs 無法移除它 — 需要 Node 就是這項技術的定義。

napi-rs 是「Rust 必須跑在既有 Node 行程**內部**」時的正確工具（oxc、SWC、Biome 的 JS bindings、Prisma 都是這個情境）。**本專案的 statusline 是獨立行程，工具選錯了。**

> **即便如此，napi-rs 的 CI 樣板仍然值得抄** — 就算不用 napi-rs 本身，那份 14-target 的交叉編譯 matrix（尤其 musl 走 zig / cargo-zigbuild 的部分）是現成可用的。

### 3.3 方案 B — 純 Rust binary 透過 npm 發佈（推薦）

**機制** — 依 [npm package.json 文件](https://docs.npmjs.com/cli/v10/configuring-npm/package-json)：
- **`optionalDependencies`**：「If a dependency can be used, but you would like npm to proceed if it cannot be found or fails to install, then you may put it in the `optionalDependencies` object.」安裝失敗不會讓整體安裝失敗。「It is still your program's responsibility to handle the lack of the dependency.」
- **`os`**：如 `["darwin","linux"]`，可用 `!` 反向，比對 `process.platform`。
- **`cpu`**：如 `["x64","arm64"]`，可反向，比對 `process.arch`。

合起來：每個平台套件宣告 `os`/`cpu`，npm 把不符的當作「failed optional」安裝略過，最後剛好落地一個。

**實測驗證**：`npm install @node-rs/argon2` 宣告 13 個 optionalDependencies，實際只裝了 `argon2-darwin-arm64` 一個。esbuild（宣告 26 個 → 只裝 `@esbuild/darwin-arm64`）與 Biome（8 → 只裝 `cli-darwin-arm64`）亦同。

**已知失敗模式**（全部有一手來源）：

1. **`--no-optional` / `--omit=optional`** — esbuild 的 [`node-platform.ts`](https://github.com/evanw/esbuild/blob/main/lib/npm/node-platform.ts) 直接報錯：「make sure that you don't specify the `--no-optional` or `--omit=optional` flags. The `optionalDependencies` feature of `package.json` is used by esbuild to install the correct binary executable for your current platform.」esbuild 的 `install.js` 以**從 registry 用 HTTPS 下載 tarball 並解壓**（`node-install.ts` 裡的 `fetch()` + `zlib`）作為補償，並用 [`npm/esbuild/package.json`](https://github.com/evanw/esbuild/blob/main/npm/esbuild/package.json) 中內嵌的 `esbuild.binaryHashes` 欄位做 SHA-256 驗證。**`--no-optional` 與 `--ignore-scripts` 同時出現時，esbuild 明確表示無解。**

2. **npm lockfile bug** — [npm/cli#4828](https://github.com/npm/cli/issues/4828)：「[BUG] Platform-specific optional dependencies not being included in `package-lock.json` when reinstalling with `node_modules` present」。在 `node_modules` 存在的情況下重新產生 lock，只會記錄**本機安裝的那個平台變體**；把那份 lock 分享出去會**無聲地**弄壞其他平台的同事。回報於 npm 8.x，已由 PR #8184 關閉。Issue 中的真實案例是 `@swc/core`。

3. **跨平台複製 `node_modules`** — esbuild 的錯誤訊息特別點名 Docker（從 macOS `COPY node_modules` 進 Linux image）與 WSL，以及 **Rosetta 2**（npm 跑在 Rosetta 下、node 不在）。

4. **Yarn** — 多平台安裝需要在 `.yarnrc.yml` 設定 [`supportedArchitectures`](https://yarnpkg.com/configuration/yarnrc/#supportedArchitectures)（napi-rs 自己的 CI 就這樣做：`yarn config set --json supportedArchitectures.cpu '["current","arm64","x64","arm"]'`）。Yarn PnP 需要 `preferUnplugged: true`（[`@esbuild/darwin-arm64`](https://unpkg.com/@esbuild/darwin-arm64@0.28.2/package.json) 有帶）；esbuild 還為舊版 PnP 保留了把 binary 複製到 `node_modules/.cache/esbuild` 的 `.zip/` 路徑 hack。

5. **musl vs glibc** — `os`/`cpu` **無法表達 libc**。所有人都在 runtime 偵測：Biome 的 [`bin/biome`](https://github.com/biomejs/biome/blob/main/packages/@biomejs/biome/bin/biome) 執行 `execSync("ldd --version")` 檢查是否含 `"musl"`；napi-rs 產生的 loader 用三種方法試。

**關鍵問題：能不能完全跳過 Node？能 — 而且這是整件事的核心。**

官方 statusline 文件確認 `statusLine.command` 接受**任何可執行檔路徑**，不限於 shell 腳本，且透過 shell 執行（所以 pipe 也能用）。三種達成方式，由好到差：

- **在 postinstall 把 binary 複製出來**（Rust statusline 生態已在做的事）。[CCometixLine](https://github.com/Haleclipse/CCometixLine) 的 `npm/main/scripts/postinstall.js` 把平台 binary 複製到 `~/.claude/ccline/`，README 指示：
  ```json
  { "statusLine": { "type": "command", "command": "~/.claude/ccline/ccline", "padding": 0 } }
  ```
  **render 路徑上零 Node。這是該抄的模式** — 已被驗證，而且同時繞開整個「node_modules 路徑穩定性」問題。
- **直接指向平台套件**：`command: "<proj>/node_modules/@you/statusline-darwin-arm64/bin/statusline"`。可行（實測 esbuild 的是 4.02 ms），但路徑內嵌了平台 triple — 對需要同步的 `settings.json` 是壞事。
- **esbuild 的手法**：把 binary 硬連結蓋過自己 `bin/` 的 JS shim，讓 `node_modules/.bin/<name>` **本身就是** binary。優雅，但繼承 esbuild 記載的那些例外（Windows、Yarn、`--ignore-scripts`）。

誠實的但書：esbuild 的 4.14 ms vs 裸 Rust binary 的 1.73 ms，差距來自 binary 大小與動態連結（esbuild 是 ~10MB 的 Go binary，連結 CoreFoundation 與 Security）。**小的靜態連結 Rust binary 會貼近 fork+exec 地板值。**

### 3.4 方案 C — WASM（wasm-bindgen / wasm-pack）

**結論：出局。四項需求中有兩項是不可能，而非只是慢。**

**WASI 是硬性能力牆。** [WASI preview1 完整介面](https://github.com/WebAssembly/WASI/blob/wasi-0.1/preview1/witx/wasi_snapshot_preview1.witx) 共 46 個函式，**沒有 spawn / exec / fork** — 唯二的 `proc_*` 是 `proc_exit` 與 `proc_raise`（都是對自己）。所以 **WASM 在 Node 的 WASI 下無法執行 `git`**。網路方面只有 `sock_accept/recv/send/shutdown`，**沒有 `sock_open`、沒有 `sock_connect`、沒有 DNS**，guest 程式碼無法**主動發起** HTTPS 連線。Node 實際使用的 [uvwasi](https://github.com/nodejs/uvwasi/blob/main/README.md) 更受限 — 43 個 call，且**完全沒實作 `sock_accept`**。

能用的：檔案透過 [`preopens`](https://nodejs.org/api/wasi.html)（host 提供的 虛擬→真實 目錄映射）、環境變數透過 `env` 選項（預設 `{}`，**不繼承**）。四項需求中的兩項。

此外 [`node:wasi`](https://nodejs.org/api/wasi.html) 是 **Stability 1 - Experimental**，且 Node 明確警告：「The current Node.js threat model does not provide secure sandboxing as is present in some WASI runtimes... do not rely on it to run untrusted code.」（`--experimental-wasi-unstable-preview1` 旗標自 v20.0.0 / v18.17.0 起[已非必要](https://nodejs.org/api/cli.html)。）

**`wasm-bindgen --target nodejs` 功能上可行但在此無意義。** 依[部署指南](https://rustwasm.github.io/docs/wasm-bindgen/reference/deployment.html)，它產出「loadable via `require`」的 CJS glue，定位為「an alternative to a native module」。它是**函式庫不是 CLI**：[`wasm-pack build --target nodejs`](https://rustwasm.github.io/docs/wasm-pack/commands/build.html) 設定 `main` key，**不產生 `bin`、沒有 shebang**。你得手寫 Node 進入點、吃下完整 20 ms，然後把每個 `git` spawn 與 HTTPS 呼叫都跨邊界 marshal 回 Node API。**加了一層邊界卻沒有移除任何成本。**

**關於 instantiation 成本，誠實地說**：**沒有官方 Node 文件** — `nodejs.org/api/webassembly.html` 不存在（[Node 文件樹](https://github.com/nodejs/node/tree/main/doc/api)中沒有 `webassembly.md`），[`globals.html#webassembly`](https://nodejs.org/api/globals.html#webassembly) 只是轉指 MDN。Node 的 module compile cache 範圍限於「a CommonJS, an ECMAScript Module, or a TypeScript module」— **WASM 不在內**。實測 ~450KB 模組的 compile+instantiate 在 1 ms 以下，所以 WASM instantiation **不是**瓶頸 — 但在能力牆面前這一點無關緊要。

esbuild 的[官方文件](https://esbuild.github.io/getting-started/)講得很直接：「The WebAssembly version is much, much slower than the native version. In many cases it is an order of magnitude (i.e. 10x) slower」，第一個原因就是「node re-compiles the WebAssembly code from scratch on every run」— **正是 per-render 的工作型態**。以及：「You should only use the WebAssembly package like this if there is no other option.」esbuild 在 `node-platform.ts` 中只把 WASM 保留給三個沒有原生 binary 的平台（`android arm`、`android x64`、`openharmony arm64`）。

（附註：任務描述中的 `https://rustwasm.github.io/wasm-bindgen/` 會 404；正確位置是 `/docs/wasm-bindgen/`。）

### 3.5 方案 D — 原生 Rust binary 基準線

**實測 1.73 ms**，對照 `/bin/echo` 的 1.47 ms fork+exec 地板值 — 也就是**約 0.3 ms 的實際程式成本**。建置設定：`opt-level="z"` + `lto=true` + `strip=true` + `panic="abort"` + `codegen-units=1` → **286 KB binary**。

實務前例吻合：[CCstatus](https://github.com/MaurUppi/CCstatus) 的 README 宣稱一個 3.1 MB 靜態建置**含網路探測**「啟動時間：< 50ms」— 這是含實際工作的保守端到端數字，仍遠在 300 ms debounce 之內。

不走 npm 的發佈方式：GitHub Releases + 安裝腳本、Homebrew、`cargo install`。代價是失去 npm 的平台選擇機制、得自己實作 — 這正是方案 B（npm 發佈 + 直接執行 binary）是同一組執行特性的更好包裝的原因。

### 3.6 方案 E — `@anthropic-ai/claude-agent-sdk`

**與 statusline 完全無關。層級錯了，方向也反了。**

[Agent SDK](https://code.claude.com/docs/en/agent-sdk/overview) 是**建構自主 agent** 的函式庫 — 提供 agent loop、工具（Read/Write/Edit/Bash/Glob/Grep/WebSearch/WebFetch）與 context 管理，在你的行程內執行。依 [TypeScript reference](https://code.claude.com/docs/en/agent-sdk/typescript)，主要 export 為 `query()`、`startup()`、`tool()`、`createSdkMcpServer()`、`listSessions()`、`getSessionMessages()`、`getSessionInfo()`、`resolveSettings()`。

逐項檢查：

| 問題 | 答案 |
|---|---|
| 有 statusline 型別 / 選項 / 輔助函式嗎？ | **沒有。** Options 介面中找不到任何 `statusLine` 欄位 |
| 有 statusline stdin payload 的 schema 嗎？ | **沒有。** 只在 [statusline 文件](https://code.claude.com/docs/en/statusline) 中記載 |
| 有即時 session 資料（transcript path、model、token/context usage）嗎？ | **沒有。** `getSessionInfo()` / `listSessions()` 讀的是**磁碟上的過往 session**（供 resume 用），不是 Claude Code 的即時 UI 狀態 |
| 有 usage / rate-limit / cost 嗎？ | **沒有。** |
| 有 `settingSources` 嗎？ | 在記載的 Options 型別中找不到。有一個 `settings` 欄位（設定檔路徑或 inline 物件）。**這是本次調研中信心度最低的一項** — 可能未記載或已改名 |
| 會讀 `~/.claude/settings.json` 的 statusLine 設定嗎？ | **不會。** |
| 值得引入嗎？ | **不。而且會有反效果** — 它是重量級依賴，會把 Node 行程塞回 render 路徑，付掉正要消除的那 ~20 ms |

方向相反：**statusline 是 Claude Code 回呼你的程式；SDK 是你的程式去呼叫 Claude。**

---

## 四、對本專案的具體建議

### 4.1 應該保留的現行設計

現行 `claudeStatusLine.sh` 有幾處**優於**多數 Rust 專案，重寫時務必保留：

| 現行作法 | 位置 | 為何優於 Rust 生態 |
|---|---|---|
| 單一 `git status --porcelain=v2 --branch --no-ahead-behind` 同時取分支與狀態 | `claudeStatusLine.sh:365-367` | CCometixLine 每次 render 開 5 個 git 行程 |
| `--no-optional-locks` | 同上 | 只有 CCometixLine 血系也有做；避免與前景 git 搶 index lock |
| 用 jq `explode` 自建寬度計算，涵蓋 zero-width、East Asian wide、emoji 範圍 | `claudeStatusLine.sh:853-884` | 比 4 個用 `.chars().count()` 的 Rust 專案準確 |
| 硬編 User-Agent `claude-code/2.1.34` | `claudeStatusLine.sh:556` | CCometixLine 為此執行 `npm view` — render 中多一次網路往返 |
| `mkdir` 當原子 lock + temp file + rename 原子寫入 | `claudeStatusLine.sh:195-208`, `358`, `543` | 與 CCstatus 的 `fs2` 檔案鎖等效，但零依賴 |
| `_dir_is_safe()` 驗證 cache 目錄擁有者與 mode bits，fail-closed | `claudeStatusLine.sh:159-193` | **沒有任何 Rust 專案做這件事** |
| 60 秒 OAuth 快取 / 2 秒 git 快取 | `claudeStatusLine.sh:516`, `309` | 符合 1.5 節的 in-flight cancellation 設計要求 |

### 4.2 應該改變的

1. **`effort.level` 改讀 stdin。** 移除 `settings.json` 讀取與 `CLAUDE_CODE_EFFORT_LEVEL` 分支（`claudeStatusLine.sh:280-296`），改用 payload 的 `effort.level`。這修正了「讀不到 session 中途 `/effort` 變更」的正確性缺陷，同時省掉一次檔案 I/O 與一次 `jq` 子行程。注意等級集合是 `low` / `medium` / `high` / `xhigh` / `max`（現行 `case` 少了 `xhigh`），且模型不支援時整個 `effort` 不存在。

2. **`context_window.used_percentage` 直接用官方值。** 現行自算公式雖然與官方一致，但用官方預算值可保證與 `/context` 對齊。

3. **serde struct 全面 `Option` + `#[serde(default)]`**，`model` 用自訂 `Deserialize` 接受 string-or-object（best-claude-hud 的作法），並加 `#[serde(flatten)] extra: serde_json::Value` 以便未來欄位不會導致解析失敗。stdin 為空時回 `Default::default()` 而非 error（statusline-pro 的作法）。

4. **每模組 timeout 用 thread + mpsc 保證**（sotayamashita 的 `timeout.rs` 模式），取代現行對 `timeout`/`gtimeout` 存在與否的依賴（`claudeStatusLine.sh:132-147`）。

5. **`[profile.release]` 從第一天就設好**：`opt-level = "z"`、`lto = true`、`codegen-units = 1`、`strip = true`、`panic = "abort"`。三個最紅的 Rust 專案都漏了這一段。

6. **`unicode-width` 是必要依賴**，不是可選。

7. **公布真實的 `hyperfine` 冷啟動數字。** 整個生態沒有人做這件事；做了就是最硬的差異化。

### 4.3 建議的發佈流程

1. Rust binary，release profile 如上。
2. 發佈 per-platform npm 套件，帶 `os`/`cpu`，在主套件列為 `optionalDependencies`。交叉編譯 matrix **直接抄 [napi-rs 的 CI.yml](https://github.com/napi-rs/package-template/blob/main/.github/workflows/CI.yml)**（尤其 musl 走 zig / `cargo-zigbuild` 的部分），即使不使用 napi-rs 本身。發佈時設 `provenance: true`。
3. **postinstall 把 binary 複製到 `~/.claude/<name>/<name>`**，README 指示 `"command": "~/.claude/<name>/<name>"`。**這是最重要的單一決策** — 它同時把 Node 移出 render 路徑，並給 `settings.json` 一個平台無關、可同步的路徑。
4. Runtime 偵測 musl（`ldd --version` 找 `"musl"`，依 Biome / napi-rs 作法）。
5. 提供 `--no-optional` 的 fallback 下載並做 hash 驗證（esbuild 的 `binaryHashes` 模式），至少要有一段明確指出 `--omit=optional` 是可能原因的錯誤訊息。
6. 保留現行的檔案快取策略，並考慮加入 CCstatus 的 COLD/RED/GREEN 探測視窗概念來調節 OAuth usage 的更新頻率。

---

## 五、待決事項（需要你決定）

1. **能否用 stdin 的 per-model 7 日視窗取代整個 OAuth 流程？**
   ccstatusline 的 schema 顯示 `rate_limits` 存在 `seven_day_sonnet` 與 `seven_day_opus`（[`src/types/StatusJSON.ts`](https://github.com/sirmalloc/ccstatusline/blob/main/src/types/StatusJSON.ts)），這兩個欄位**官方文件未記載**。目前腳本為了取得 Fable 週額度，必須做 keychain 讀取 + OAuth 呼叫 + 60 秒快取（`claudeStatusLine.sh:477-565`, `726-749`）— 這是整個腳本最複雜、最脆弱、也最有安全顧慮的部分。
   **如果 payload 裡存在 `seven_day_fable` 之類的欄位，這一整塊可以全部刪掉。**
   驗證方法很直接：把 `settings.json` 的 statusLine 暫時改成 `"command": "cat > /tmp/cc-payload.json"`，在有 Fable 額度的 session 中觸發一次，然後檢查該檔案。**建議在動工前先做這件事，它可能大幅改變專案範圍。**

   > **✅ 已驗證（2026-08-22，Claude Code v2.1.233，Fable 5 session，帳號確有 Fable 週額度）：**
   > 以 `tee` 側錄實際 payload。`rate_limits` **只有** `five_hour` 與 `seven_day` 兩個欄位（`used_percentage` + epoch 秒數的 `resets_at`），**沒有** `seven_day_sonnet` / `seven_day_opus`，也**沒有任何 per-model（Fable）週額度欄位**。結論：**假設不成立，OAuth 流程必須保留**（只要還想顯示 Fable 週額度與 extra usage）。
   > 同次側錄一併確認：`effort.level` 存在（4.2 節第 1 點成立）；`used_percentage` / `remaining_percentage` 為預算值；`context_window_size` 回報 1000000（1m context session），重寫必須沿用 payload 值而非硬編 200k；另有 `session_name`、`workspace.repo`（host/owner/name）、`cost.*`、`exceeds_200k_tokens`、`fast_mode`、`thinking.enabled`、`vim.mode`、`prompt_id` 等欄位可供未來功能使用。

2. **是否維持對 Windows 的支援？**
   現行 README 提供 Git Bash 設定。Rust 版可以原生支援 Windows（`x86_64-pc-windows-msvc` / `aarch64-pc-windows-msvc`），但 keychain 憑證讀取在 Windows 上沒有對應機制（現行腳本只支援 macOS `security` 與 Linux `secret-tool`）。若第 1 點成立、OAuth 整塊被移除，這個問題也一併消失。

3. **npm 是唯一發佈通道，還是要並行 Homebrew / `cargo install` / GitHub Releases？**
   statusline-pro 用 cargo-dist 同時產出四種通道，代價是多一層工具鏈。單走 npm 最省事，但 `cargo install` 對 Rust 使用者是很自然的期待。

4. **是否順帶實作 `subagentStatusLine`？**
   同一個 binary 加一個 `--subagent` 子命令即可（見 1.7 節）。這是低成本的功能擴充，但會擴大測試面。

5. **Fable 週額度的顯示邏輯要不要泛化？**
   現行實作把 `scope.model.display_name == "fable"` 硬編（`claudeStatusLine.sh:733`）。若改成掃描 `limits[]` 中所有 `kind == "weekly_scoped"` 條目並全部顯示，就不需要為每個新模型改程式碼 — 但在有多個 scoped 限制的帳號上會讓 statusline 變長。

6. **`docs/research/` 下已有的四份研究文件要如何處理？**
   `best-claude-hud-layout-and-icons.md`、`claude-statusline-meter-layout.md`、`fable5-weekly-and-dot-ui.md`、`git-status-stage-performance.md` 記錄了現行 UI 與 git 策略的決策依據。重寫時這些決策應該原樣繼承（本文第 4.1 節已確認 git 策略優於 Rust 生態），但文件本身需要決定是保留在原處、或移進新專案結構。

7. **授權來源鏈的顧慮。**
   CCometixLine **沒有 LICENSE 檔**（僅 Cargo.toml 宣稱 MIT），而 best-claude-hud 從它 fork（51 個 `.rs` 檔中 27 個位元組相同）、改授權為 Apache-2.0、且未署名。**如果打算直接借用該血系的程式碼，來源鏈是混濁的。** 借用「概念」（增量快取設計、regex 模型家族的想法）沒有問題；直接複製程式碼則需要謹慎。本文列出的所有「值得抄」項目，建議以重新實作而非複製的方式進行。

---

## 附錄：一手來源清單

**Claude Code 官方文件**
- statusline 主頁：<https://code.claude.com/docs/en/statusline>
- settings reference（原始 Markdown）：<https://code.claude.com/docs/en/settings-reference.md>
- Agent SDK overview：<https://code.claude.com/docs/en/agent-sdk/overview>
- Agent SDK TypeScript reference：<https://code.claude.com/docs/en/agent-sdk/typescript>

**Rust statusline 專案原始碼**
- <https://github.com/Haleclipse/CCometixLine>
- <https://github.com/GaoSSR/best-claude-hud>
- <https://github.com/Wangnov/claude-code-statusline-pro>
- <https://github.com/MaurUppi/CCstatus>
- <https://github.com/sotayamashita/claude-code-statusline>
- <https://github.com/khoi/cc-statusline-rs>
- <https://github.com/ding113/ccline-packycc>
- <https://github.com/sirmalloc/ccstatusline>（TypeScript 對照組）

**發佈與整合**
- napi-rs 文件：<https://napi.rs/docs/introduction/getting-started>、<https://napi.rs/docs/cli/build>、<https://napi.rs/docs/cli/artifacts>
- napi-rs CI 樣板：<https://github.com/napi-rs/package-template/blob/main/.github/workflows/CI.yml>
- esbuild 安裝實作：<https://github.com/evanw/esbuild/blob/main/lib/npm/node-install.ts>、<https://github.com/evanw/esbuild/blob/main/lib/npm/node-platform.ts>
- esbuild 官方文件：<https://esbuild.github.io/getting-started/>
- Biome bin wrapper：<https://github.com/biomejs/biome/blob/main/packages/@biomejs/biome/bin/biome>
- npm package.json 規格：<https://docs.npmjs.com/cli/v10/configuring-npm/package-json>
- npm optionalDependencies lockfile bug：<https://github.com/npm/cli/issues/4828>
- Yarn supportedArchitectures：<https://yarnpkg.com/configuration/yarnrc/#supportedArchitectures>

**Node.js / WASM**
- Node startup benchmark：<https://github.com/nodejs/node/blob/main/benchmark/misc/startup-core.js>、<https://github.com/nodejs/node/blob/main/benchmark/misc/startup-cli-version.js>
- Node module compile cache：<https://nodejs.org/api/module.html#module-compile-cache>
- Node WASI：<https://nodejs.org/api/wasi.html>
- uvwasi：<https://github.com/nodejs/uvwasi/blob/main/README.md>
- WASI preview1 規格：<https://github.com/WebAssembly/WASI/blob/wasi-0.1/preview1/witx/wasi_snapshot_preview1.witx>
- wasm-bindgen 部署指南：<https://rustwasm.github.io/docs/wasm-bindgen/reference/deployment.html>
- wasm-pack build：<https://rustwasm.github.io/docs/wasm-pack/commands/build.html>
