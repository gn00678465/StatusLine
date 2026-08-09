# Changelog

本專案所有重要變更都記錄在這個檔案。

格式遵循 [Keep a Changelog](https://keepachangelog.com/zh-TW/1.1.0/)，
版號採 [Semantic Versioning](https://semver.org/lang/zh-TW/spec/v2.0.0.html)：
**MAJOR.MINOR.PATCH** = 不相容變更 / 新功能 / 修正。

## [Unreleased]

## [v1.2.1] - 2026-08-09

### Changed

- **Usage meter spacing**：dot 模式的 10 個 `●` / `○` 位置各以一個空格分隔；bar 模式維持連續 `▓░` 色塊
- **Semantic emoji**：既有布局加入 `🤖` model、`🧠` effort、`⚡️` context 與單一 `📊` rate-limit group；原有 `📁` workspace、`🌿` Git 保持不變
- **Fable label**：將不易辨識的 `F5` 改為完整名稱 `Fable 5`
- **Git staged/working tree 語意**：以單次 porcelain v2 snapshot 取代 branch + `diff --numstat`；`S` / `W` / `C` 顯示 staged、unstaged、conflict 檔案數，修正 staged-only 變更完全不可見與前 200 檔截斷造成的錯誤統計

### Performance

- **Git refresh 降載**：所有背景查詢使用 `--no-optional-locks`，略過 untracked、submodule dirty 與 rename similarity 掃描，並加入預設 2 秒、可用 `STATUSLINE_GIT_CACHE_TTL` 調整或停用的短效 cache

### Notes

- 無 breaking change；直接更新腳本即可

## [v1.2.0] - 2026-08-09

### Added

- **Fable 5 weekly usage**：從 Anthropic OAuth usage response 的 dynamic `limits[]` 中取得 `weekly_scoped` / `Fable` 使用率與 reset time，並以 `F5` 區塊顯示；帳號無此 scope 或 API 失敗時靜默省略
- **統一 10-dot usage UI**：設定 `STATUSLINE_USAGE_STYLE=dots` 即可同時將 context、5h、7d 與 Fable weekly 切換為 `●○` 顯示；未設定或其他值維持原有 bar
- **Mock evaluation**：覆蓋 built-in rate limits、OAuth fallback、Fable scoped limit、bar/dots 切換、10-dot 數量、四段顏色與 missing-scope fallback

### Changed

- 在 Claude Code stdin 已提供 5h/7d 時仍會透過現有 60 秒 cache 讀取 OAuth usage，以取得 stdin 缺少的 per-model weekly limit

### Documentation

- 記錄 Fable weekly 資料流、OAuth response shape，以及參考 dot UI 的 glyph、rounding 與顏色語意

### Notes

- 無 breaking change；現有使用者不設定 `STATUSLINE_USAGE_STYLE` 時保持 bar 顯示

## [v1.1.3] - 2026-05-30

### Fixed

- **bash 3.2 相容性**：macOS 內建 `/bin/bash`（3.2.57）無 `mapfile`（4.0+），三處 JSON 解析會噴 `mapfile: command not found` 並退化輸出（model 變 `Claude`、token 變 0、rate-limit 區塊消失）。改用 `while IFS= read -r ... || [ -n "$line" ]` 迴圈逐行讀入陣列，保留空行並補捉無結尾換行的最後一行，行為與 `mapfile -t` 等價。此項為 v1.1.2 `Notes` 中列為暫不處理的 Low 相容性項目。

## [v1.1.2] - 2026-05-06

### Security

合併三輪 copilot 安全審查的修補。第三輪由 gpt-5.4 + gpt-5.4 sub-reviewer 確認 R1+R2 修補有效，並抓出 R2 cache_dir fix 的兩個邏輯瑕疵：

**Round 2 → Round 3 修補的 R2 fix 缺陷：**

- **MED** `cache_dir` 驗證順序錯誤：原本 R2 fix 是 `mkdir -p` / `chmod 700` 之後才驗證 `-L`/`-O`/`-d`。攻擊者預先 symlink 該路徑時，`chmod` 會 follow symlink 到 victim 檔案。R3 修補：先驗證 `_cache_base`（HOME / XDG_RUNTIME_DIR）必須是 user-owned 真目錄，**通過後**才 `mkdir` 子目錄；mkdir 後再驗一次（防 mkdir 過程被搶）
- **MED** `_cache_safe=false` 沒有真的 fail-closed：原本只擋 `_atomic_write`，但腳本仍從 `cache_dir` 讀 cache 檔（接收攻擊者餵的假內容）+ 建 `*.lock` 目錄（暴露活動）。R3 修補：**所有** disk read、lock create/remove、stat 操作都 gate 在 `_cache_safe`；不安全時 `cache_dir` 設為 `/dev/null` 防呆，所有 cache 路徑變成記憶體 only

**Round 2 修補（已確認有效）：**

- `effort_level` 從 `CLAUDE_CODE_EFFORT_LEVEL` env / `settings.json` 取出後加 `tr -d '\000-\037\177'` + `[a-zA-Z]` whitelist + 16 字元截斷
- `safe_str` 擴充含 BiDi/zero-width Unicode 控制字元範圍

**Round 1 修補（已確認有效）：**

- OAuth bearer token 不再透過 `curl -H "Authorization: Bearer $token"` 暴露在 process argv，改用 `printf ... | curl --config -` 從 stdin 讀 config（R3 子任務獨立驗證 curl config 解析，確認 token 含換行/引號也不會 inject directives）
- JSON 數值欄位透過 `tonumber? // 0` + `_num()` 雙重驗證，杜絕 `$(( ... ))` recursive expansion 觸發 command substitution
- 共享 `/tmp/claude/` 改成 per-user，所有 cache write 改用 `mktemp + mv` 原子替換
- 字串欄位透過 jq `gsub` 移除控制字元
- 最終輸出 `printf "%b"` → `printf "%s"`；ANSI 顏色用 `$'\033...'` 直接內嵌真實 ESC byte
- `--connect-timeout 3` 限制 DNS / TCP stall
- `cache_mtime` 雙 stat 失敗時 fallback 0
- `latest_tag` 限制版號合法字元
- `git diff --numstat` 加 `timeout 3` + `head -n 200`
- `mkdir` atomic lock 防 cache refresh 並發競態
- `COLUMNS` regex 驗證
- macOS `security` 加 `timeout 3`

### Round 4 修補（chmod-after-validation race）

- 引入 `_dir_is_safe()` helper 檢查 not-symlink、is-dir、owned-by-us、**no group/other write bit**（不只 ownership）
- 移除 `chmod 700` after validation：`umask 077 + mkdir` 直接產生 0700 dir，杜絕 chmod-then-swap race
- 移除 `/tmp/claude-${UID}` fallback：沒有 safe base 時 `_cache_safe=false`，純記憶體模式

### Round 5 修補（ancestor chain + numeric coercion）

- **MED**：`$HOME/.cache` 既存分支只驗 `.cache` 自己，沒驗 `$HOME` 是否安全。修補：要求 `$HOME` 與 `$HOME/.cache` **都** 通過 `_dir_is_safe`（不論 `.cache` 是預先存在還是新建）
- **MED**：builtin rate-limit `used_percentage` 用 `tostring` 取出後直接餵 `printf -v "%.0f"`，malformed JSON 可觸發 invalid-number 診斷把 attacker bytes 灑到 stderr。修補：先過 `_num "${var%%.*}"` 強制成整數，0 表示無資料

### Threat Model 限制

- **mode-bit 邏輯**：`_dir_is_safe()` 只看傳統 Unix mode bits。在啟用 POSIX/NFSv4 ACL 的檔案系統上，dir 可能 mode 顯示 `0700` 但 ACL 仍允許他人寫入。Cross-platform ACL inspection 在純 bash 不可行；此檔系上的 hostile-host 部署需要 operator 確保 `$HOME` / `$XDG_RUNTIME_DIR` 沒有 extended write ACL
- **Signal handling**：`SIGINT/SIGPIPE/SIGHUP` 在持鎖期間打斷會留下 30s 內 stale lock dir（自動清除）。LOW，非 must-fix

### Notes

- **單機單用戶受信任 shell**：production-ready，可以 ship
- **多用戶共享主機 hostile env**（無 extended ACL）：production-ready，可以 ship
- **多用戶共享主機 + extended ACL 啟用**：documented limitation，需要 operator 確認 ACL 配置
- 升級不影響 settings.json
- cache 位置從 `/tmp/claude/` 移到 `${XDG_RUNTIME_DIR}/StatusLine` 或 `${HOME}/.cache/StatusLine`；首次執行重建
- 暫不處理的 Low：`COLUMNS` 沒上限、無 SIGINT trap、bash 3.2 / GNU timeout / jq <1.6 相容性

## [v1.1.1] - 2026-05-06

### Security

依 Copilot CLI（GPT-5.4 + Claude sub-reviewer）安全審查報告修補三項主要問題：

- **CRIT** OAuth bearer token 不再透過 `curl -H "Authorization: Bearer $token"` 暴露在 process argv（`/proc/<pid>/cmdline`、`ps auxww` 可見）。改用 `printf 'header = "Authorization: Bearer %s"' "$token" | curl --config -` 從 stdin 讀 config，token 永不進入 argv
- **CRIT** Untrusted JSON 數值不再直接進入 bash arithmetic（`$(( ... ))` 對變數做 recursive expansion 包含 command substitution）。jq 端用 `tonumber? // 0` 強制成數字；shell 端用 `_num()` regex 驗證 `^[0-9]+$`，雙層 defense
- **HIGH** 共享 `/tmp/claude/` 改成 per-user `/tmp/claude-${UID}/`（mode 700），杜絕本機 symlink 攻擊（攻擊者預先在共享目錄放 symlink 讓 `> file` 跟隨指向受害者檔案）。所有 cache / state 寫入改用 `mktemp + mv` 原子替換

### Changed

- **Hardening**：所有 user-controlled 字串欄位（`model.display_name`、`session_id`、`cwd`、`resets_at`）在 jq 端透過 `gsub("[\\u0000-\\u001f\\u007f]"; "")` 移除控制字元，杜絕 terminal escape 注入
- **Hardening**：最終輸出從 `printf "%b"` 改成 `printf "%s"`；ANSI 顏色定義改用 `$'\033...'` 直接內嵌真實 ESC byte。user-controlled 字串中的 `\033`、`\xHH`、`\uHHHH` 不再被 printf 解譯
- **Hardening**：`latest_tag` 額外用 `tr -dc 'a-zA-Z0-9.+-'` 限制為版號合法字元
- 補上 `curl --connect-timeout 3`，避免 DNS / TCP stall 吃滿 `--max-time`
- `cache_mtime` 雙 stat 都失敗時退回 `0`，避免空字串導致 arithmetic syntax error 洩漏到 stderr
- `claude_config_dir_hash` 截斷到 64 字元，避免極長路徑超過檔名限制

### Notes

- **無 breaking change**：升級僅需替換腳本，settings.json 不需調整
- 既有 cache 檔位置從 `/tmp/claude/` 移到 `/tmp/claude-${UID}/`，舊檔案會被忽略；首次執行會重建（一次 OAuth API call）
- Windows / MSYS 上 `chmod 700` 因檔系限制不會生效，但 Windows TEMP 預設 ACL 已是 per-user，實質安全等價

## [v1.1.0] - 2026-05-06

### Added

- **Cache 命中率 + TTL 倒數區塊**：在 token 使用量之後、5h rate limit 之前顯示 `Cache 94% 56:41` 樣式
  - 命中率 = `cache_read / (input + cache_creation + cache_read)`，綠 ≥50% / 灰 <50%
  - 低命中率有助於識別 cache_control header 被破壞的中轉站／API 代理
  - TTL 從上次響應起算 1 小時；signature 機制確保只在 token 變動時重置
  - 顏色階段：0–20m 綠 / 20–40m 黃 / 40–55m 紅 / 最後 5m 閃紅 / 過期 `exp` 灰
  - 多 session 隔離：state 檔按 sanitized session_id 命名
  - State 檔損壞自動重置；首次響應前 fallback 顯示上一次的命中率

### Performance

- **端到端執行時間 ~37s → ~4s（~9x 加速）**，主要受益於 Windows Git Bash / MSYS 環境
- 子進程數從 ~50 個降到 ~5 個：
  - 三次合併 jq 呼叫（input / usage_data / state）取代 22+ 次零散呼叫
  - jq 用 `localtime + months 陣列` 取代 `strftime("%b ...")`，避開 MSYS jq locale 月份名亂碼
  - bash 算術取代 awk（hit_rate、format_tokens）
  - bash extglob 取代 sed 剝除 ANSI 控制碼
  - `usage_color` / `generate_bar` 改寫為 inline 變體，省掉 `$(...)` 子 shell
  - 純 bash loop 取代 awk 統計 `git diff --numstat`
  - session_id 直接 sanitize，不走 `sha256sum`
  - 重用 cached `$now`，避免多次 `date +%s`
- 修正 `mapfile -t` 在 Git Bash 下殘留 `\r` 的問題（jq 輸出 CRLF）

### Documentation

- 檔頭「預期效果範例」更新加入 Cache 區塊與顏色階段說明

### Notes

- 無 breaking change，現有使用者升級不需調整 `settings.json`
- 不要在 `settings.json` 加 `"refreshInterval": 1`——這不是 Claude Code 官方 statusLine 鍵，會讓整個 statusLine 區塊失效

## [v1.0.0] - 2026-04-30

### Added

- 初始 status line 實作，顯示工作目錄、Git 分支、模型名稱、effort 等級、context window 用量、5h / 7d / extra rate limits
- 支援動態折行（單行 / 雙行）依終端機寬度自動切換
- 內建版本檢查：對比 GitHub releases 最新 tag，顯示更新提示

[Unreleased]: https://github.com/gn00678465/StatusLine/compare/v1.2.1...HEAD
[v1.2.1]: https://github.com/gn00678465/StatusLine/compare/v1.2.0...v1.2.1
[v1.2.0]: https://github.com/gn00678465/StatusLine/compare/v1.1.3...v1.2.0
[v1.1.3]: https://github.com/gn00678465/StatusLine/compare/v1.1.2...v1.1.3
[v1.1.2]: https://github.com/gn00678465/StatusLine/compare/v1.1.1...v1.1.2
[v1.1.1]: https://github.com/gn00678465/StatusLine/compare/v1.1.0...v1.1.1
[v1.1.0]: https://github.com/gn00678465/StatusLine/compare/v1.0.0...v1.1.0
[v1.0.0]: https://github.com/gn00678465/StatusLine/releases/tag/v1.0.0
