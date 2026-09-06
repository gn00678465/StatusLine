# SPEC — 使用者設定檔 `~/.config/cc-statusline/config.toml` (Tier 2)

- `spec_version`: v3
- `status`: approved
- `tier`: 2
- `scope`: `config-file`
- 研究依據: `docs/research/user-config-file-conventions.md`

## 背景與目標

cc-statusline 目前只從環境變數讀取設定（`src/config.rs`）。使用者以 chezmoi 管理
`~/.claude/settings.json`，每次 `chezmoi update` 都會把本機加上的
`STATUSLINE_USAGE_STYLE=dots` 重設。本變更新增一個獨立的使用者設定檔，
讓設定不再寄生於 Claude Code 的 `settings.json`。

## 設計決定

| 項目 | 決定 | 依據 |
| --- | --- | --- |
| 路徑 | `$XDG_CONFIG_HOME/cc-statusline/config.toml`；`XDG_CONFIG_HOME` 未設或為空時用 `~/.config/cc-statusline/config.toml`。家目錄沿用 `cachedir::home_directory()`（Windows 讀 `USERPROFILE`，再退回 `HOME`；其他平台讀 `HOME`）。所有平台同一路徑，不用 `%APPDATA%`／`Application Support` | 研究 §2、§9.1：starship／bat／ccstatusline 皆用 `~/.config`；與既有 `XDG_RUNTIME_DIR` 快取慣例一致 |
| 格式 | TOML，解析器用 `basic-toml`（dtolnay 維護，只依賴已存在的 `serde`） | 研究 §4：需要註解；`serde_json` 不支援註解；實測 `basic-toml` 只新增 1 個 crate，`toml` 最小 feature 仍新增 5 個 |
| 鍵 | `usage_style`（字串 `"bar"`／`"dots"`）、`git_cache_ttl`（整數 0–60）。`COLUMNS` 是終端屬性，**不**從檔案讀 | 對應現有兩個 `STATUSLINE_*` 環境變數 |
| 優先序 | 逐鍵決定：環境變數（非空）＞設定檔＞內建預設。空字串環境變數視為未設定 | 研究 §5（clig.dev、cargo、starship） |
| 無效值 | 選中的來源若值無效（如 `usage_style = "foo"`、`git_cache_ttl = 99`），套用與現有環境變數相同的規則：`foo`→`bar`；`99`→夾到 `60`；不再往下一層找 | 與 `parse_usage_style`／`parse_git_cache_ttl` 現行語意一致 |
| 檔案不存在 | 靜默套用預設，不印任何訊息 | 研究 §6 starship 模式 |
| 檔案無法讀取或格式錯誤 | 整份檔案忽略，套用預設；在 **stderr** 印一行 `cc-statusline: ignoring config file <path>: <error>`；stdout 照常輸出 statusline；exit 0 | 研究 §6、§9.4；AGENTS.md「render path 必印至少 `Claude` 並 exit 0」 |
| 型別錯誤 | `usage_style = 1` 或 `git_cache_ttl = -1`、`git_cache_ttl = "2"` 等型別不符視為「格式錯誤」（整份忽略＋stderr） | `serde` 型別反序列化語意 |
| 未知鍵 | 忽略（不用 `deny_unknown_fields`） | 研究 §7.1：chezmoi 跨機器同步時版本可能不一致 |
| 寫入 | 本 binary 永不建立或寫入設定檔與其目錄 | 讀取路徑不涉及安全目錄檢查 |
| 建立者 | 設定檔由使用者自行建立（手動或經 chezmoi）。binary 不自動建立範本，也不提供 `--init-config`；README 提供可直接複製的範例 | 審閱 Q7 採 A：render 路徑每秒執行且必須 exit 0，寫家目錄會增加失敗模式並與 chezmoi 的檔案擁有權衝突；starship 同樣不自動建立 |
| 資源上限 | 設定檔最多讀取 16384 bytes（16 KiB）；超過即視為「無法讀取」：整份忽略＋stderr diagnostic（訊息含路徑與上限）。TOML 解析在專用 thread 上執行，stack 固定 16 MiB，使 16 KiB 內任何巢狀深度都不可能溢位 | 審查 F1（codex-astra）：實測 `basic-toml` 在 1 MiB 主執行緒 stack 於巢狀深度 2500（約 5 KB 檔案）溢位並中止程序、stdout 為空，違反 Must NOT 第 3 條；16 MiB thread 實測深度 32768（64 KB）仍可解析，16 KiB 上限保留約 5 倍餘裕 |

設定檔範例（將寫入 README）：

```toml
# ~/.config/cc-statusline/config.toml
usage_style = "dots"   # "bar" (預設) 或 "dots"
git_cache_ttl = 2      # 0–60 秒，Git 狀態快取
```

## Scenarios

每個 scenario 對應一個同名自動化測試。單元測試在 `src/config.rs`；整合測試在
`tests/cc_statusline.rs`，實際執行編譯後的 binary。

單元（`src/config.rs`）：

1. `file_usage_style_dots_applies_when_env_absent`：env 無 `STATUSLINE_USAGE_STYLE`，檔案 `usage_style = "dots"` → `UsageStyle::Dots`。
2. `env_usage_style_overrides_file`：env `bar`，檔案 `dots` → `Bar`；env `dots`，檔案 `bar` → `Dots`。
3. `empty_env_value_is_absent_so_file_applies`：env `STATUSLINE_USAGE_STYLE=""`，檔案 `dots` → `Dots`；env `STATUSLINE_GIT_CACHE_TTL=""`，檔案 `git_cache_ttl = 7` → `7`。
4. `file_git_cache_ttl_applies_and_clamps_to_60`：檔案 `git_cache_ttl = 7` → `7`；`= 99` → `60`；`= 0` → `0`。
5. `invalid_file_usage_style_falls_back_to_bar`：檔案 `usage_style = "foo"` → `Bar`，且無 diagnostic（值無效不是格式錯誤）。
6. `unknown_file_keys_are_ignored`：檔案含 `future_key = true` 與 `usage_style = "dots"` → `Dots`，無 diagnostic。
7. `file_cannot_set_columns`：檔案 `columns = 50`，env `COLUMNS` 未設 → `columns() == 100`。
8. `missing_config_file_yields_defaults_without_diagnostic`：tempdir 內不存在的路徑 → 全預設（`Bar`、`2`、`100`），diagnostic 為 `None`。
9. `malformed_config_file_yields_defaults_with_diagnostic`：檔案內容分別為 `usage_style = ` (語法錯誤)、`usage_style = 1`、`git_cache_ttl = -1`、`git_cache_ttl = "2"` → 每一種都全預設，且 diagnostic 為 `Some(訊息)`，訊息包含檔案路徑。
10. `unreadable_config_path_yields_defaults_with_diagnostic`：路徑是目錄而非檔案 → 全預設，diagnostic `Some`。
11. `config_path_prefers_xdg_config_home_then_home_dot_config_then_none`：`XDG_CONFIG_HOME=/x` → `/x/cc-statusline/config.toml`；`XDG_CONFIG_HOME` 為空且 home=`/h` → `/h/.config/cc-statusline/config.toml`；兩者皆無 → `None`（→ 全預設、無 diagnostic）。
12. `env_only_behaviour_is_unchanged_without_file`：無檔案時，現有 `parses_usage_style_ttl_and_columns_with_clamps_and_fallbacks` 的全部斷言維持不變（保留該測試不動）。

整合（`tests/cc_statusline.rs`，真實 binary、隔離 home，不觸網路）：

13. `renders_dots_meter_from_config_file`：tempdir 作為 `HOME`／`USERPROFILE`／`XDG_CONFIG_HOME`，寫入 `usage_style = "dots"`，移除 `STATUSLINE_*`／`CLAUDE_CODE_OAUTH_TOKEN`，`CLAUDE_CONFIG_DIR` 指向空 tempdir，預先寫入新鮮的 `.cache/StatusLine/statusline-version-cache.json` 以避開更新檢查的網路請求；stdin 餵 `tests/fixtures/status-input.json` → exit 0；stdout 含 `●` 或 `○`；stderr 為空。
14. `renders_status_line_and_warns_on_malformed_config_file`：同上隔離，但檔案內容 `usage_style = 1` → exit 0；stdout 含 `Fable 5` 與 `▓`／`░`（bar 預設）；stderr 含 `config.toml`。
15. `env_overrides_config_file_in_real_binary`：檔案 `dots`，env `STATUSLINE_USAGE_STYLE=bar` → stdout 含 `▓`／`░`、不含 `●`。

v3 新增（資源上限）：

16. `oversized_config_file_yields_defaults_with_diagnostic`（單元）：16385 bytes 的檔案（`usage_style = "dots"` 後以註解填滿）→ 全預設、diagnostic `Some` 且含路徑；16384 bytes 的同構檔案 → `Dots`、diagnostic `None`。
17. `deeply_nested_config_file_does_not_crash`（整合，真實 binary）：檔案內容 `x = ` 後接 4000 層 `[`／`]`（約 8 KB，合法 TOML、未知鍵）→ exit 0；stdout 含 `Fable 5` 與 `▓`／`░`；stderr 為空。
18. `renders_with_defaults_when_no_home_directory_exists`（整合，真實 binary；S11「兩者皆無」的真實執行對應）：移除 `HOME`／`USERPROFILE`／`XDG_CONFIG_HOME` → exit 0；stdout 含 `Fable 5` 與 `▓`／`░`；stderr 為空。

## Must NOT

- Must NOT 改變「沒有設定檔且環境變數不變」時的任何輸出：現有 72 個測試（含 `src/main.rs` 的 inline snapshot）一字不改地通過。
- Must NOT 在 stdout 印出 statusline 以外的任何文字；diagnostic 只走 stderr。
- Must NOT 因設定檔的任何內容（語法錯誤、型別錯誤、超大、非 UTF-8、路徑是目錄、無權限）而 panic、exit 非 0 或 stdout 為空。
- Must NOT 從設定檔讀取 `COLUMNS`。
- Must NOT 建立、寫入或刪除 `~/.config` 下任何檔案或目錄。
- Must NOT 更動 `CLAUDE_CONFIG_DIR`、OAuth 憑證、快取目錄的既有語意與路徑。
- Must NOT 新增 `basic-toml` 以外的任何 crate；`Cargo.lock` 的新增項目只能是 `basic-toml`。
- Must NOT 在 render 熱路徑加入新的子程序、網路請求或多於一次的設定檔讀取。
- Must NOT 修改 `tests/fixtures/` 下既有 fixture 或現有測試的斷言。
- Must NOT 變更 `Cargo.toml` 版號（版號調整屬發佈流程，另案處理）。

## Setup plan

核准本 spec 即一次授權下列全部項目。

- 工具安裝（寫入 `~/.cargo/bin`，供 gate 的 coverage 與 mutation 層）：
  - `rustup component add llvm-tools-preview`
  - `cargo install cargo-llvm-cov --locked`
  - `cargo install cargo-mutants --locked`
- Git 隔離：分支 `feat/config-file`（自 `main` `b805014` 建立，於同一工作樹）。Commit 節奏：spec 核准時一次；每個行為 RED（只含測試）與 GREEN（只含實作）各一次；文件、gate 工具、evidence、archive 各一次。
- Gate 新增檔案（產品路徑）：`tools/gate.sh`（入口，依序跑 tests → types → lint/format → suite health → changed-line coverage → mutation → real execution → supply chain → source state）與其 helper `tools/gate/*.sh`；`.gitignore` 新增 `/.gate/`；最終 evidence 提交至 `.scratch/config-file/evidence.md`。
- 新依賴：`basic-toml = "0.1"`（dtolnay；TOML 唯讀解析；只依賴已存在的 `serde`；`cargo tree` 實測淨增 1 個 crate）。理由：設定檔需要註解，`serde_json` 不支援；`toml` crate 最小 feature 淨增 5 個 crate。
- 文件：README「Configuration」加入設定檔說明與範例；`CHANGELOG.md` `[Unreleased]` 加入 Added；`docs/research/user-config-file-conventions.md` 入版控。
- 程式碼結構：`src/config.rs` 新增 `FileConfig`（serde、全欄位 `Option`、`#[serde(default)]`）、`config_file_path()`、`read_config_file()`、`Config::load()`（系統來源）與 `Config::resolve()`（純函式，供測試注入）；`src/main.rs` 改呼叫 `Config::load()` 並把 diagnostic 以 `writeln!(stderr)`（忽略寫入錯誤）印出；`cachedir::home_directory` 改為 `pub(crate)` 供 config 共用。

## Approval

Append-only。每個核准版本一筆：逐字引用核准語句、日期、綁定的 `spec_version`。

- 2026-09-06 — approves v2 — 「核准 spec v2」（使用者於終端輸入；審閱頁留言串 `cmt_mtpsl4iv` 已於同日解決，Q1–Q7 裁定見 Revisions）
- 2026-09-06 — approves v3 — 「核准 spec v3」（使用者於終端輸入；v3 內容：資源上限決定、Scenario 16–18）

## Revisions

Append-only。

- 2026-09-06 — v3（revised-pending-approval）：獨立 code review（codex-astra）finding F1 指出設定檔無大小與巢狀深度上限；實測重現 `basic-toml` 於 1 MiB 主執行緒在深度 2500 溢位中止、stdout 為空（違反 Must NOT 第 3 條）。新增設計決定「資源上限」（16 KiB 讀取上限＋16 MiB 專用解析 thread）與 Scenario 16、17；gate 的 changed-line coverage 指出 `Config::load` 的「無家目錄」分支未被真實執行覆蓋，補 Scenario 18（S11 的真實 binary 對應，不新增行為）。其餘 review finding（F2 整合測試 PATH 隔離、F3／F4 S6／S7 測試未經 TOML 解析、F5 README 措辭）為既有 spec 條文的遵循性修正，不改版。v2 的核准不涵蓋本版；待重新核准。

- 2026-09-06 — v2：審閱頁探索第 1 輪（留言串 `cmt_mtpsl4iv`）。使用者裁定：Q1 路徑採 A（`~/.config/cc-statusline/config.toml`，尊重 `XDG_CONFIG_HOME`）、Q2 採 A（TOML + `basic-toml`）、Q3 採 A（格式錯誤 stderr 一行、套預設、exit 0）、Q4 採 A（env 勝出後值無效即套預設）、Q5 採 A（不動版號）、Q6 採 A（Tier 2）；新增 Q7「設定檔由誰建立」，使用者裁定採 A（使用者自行建立，binary 永不寫入）。決定表新增「建立者」列。v1 尚未核准，本版取代 v1 待核准。
- 2026-09-06 — v1 草稿：依 `docs/research/user-config-file-conventions.md` 定案路徑、格式、優先序與容錯策略；以 `cargo tree` 實測 `basic-toml`（淨增 1 crate）與 `toml` 最小 feature（淨增 5 crate）後選 `basic-toml`。
