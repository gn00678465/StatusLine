# Changelog

本專案所有重要變更都記錄在這個檔案。

格式遵循 [Keep a Changelog](https://keepachangelog.com/zh-TW/1.1.0/)，
版號採 [Semantic Versioning](https://semver.org/lang/zh-TW/spec/v2.0.0.html)：
**MAJOR.MINOR.PATCH** = 不相容變更 / 新功能 / 修正。

## [Unreleased]

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

### Notes

- **單機單用戶受信任 shell**：production-ready，可以 ship
- **多用戶共享主機 hostile env**：經 R3 修補後，cache fallback 路徑也已 fail-closed，可以 ship
- 升級不影響 settings.json
- cache 位置從 `/tmp/claude/` 移到 `${XDG_RUNTIME_DIR}/StatusLine` 或 `${HOME}/.cache/StatusLine`；首次執行重建
- 暫不處理的 Low（R3 確認不升級到 Medium+）：`COLUMNS` 沒上限、無 SIGINT trap、bash 3.2 / GNU timeout / jq <1.6 相容性

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

[Unreleased]: https://github.com/gn00678465/StatusLine/compare/v1.1.2...HEAD
[v1.1.2]: https://github.com/gn00678465/StatusLine/compare/v1.1.1...v1.1.2
[v1.1.1]: https://github.com/gn00678465/StatusLine/compare/v1.1.0...v1.1.1
[v1.1.0]: https://github.com/gn00678465/StatusLine/compare/v1.0.0...v1.1.0
[v1.0.0]: https://github.com/gn00678465/StatusLine/releases/tag/v1.0.0
