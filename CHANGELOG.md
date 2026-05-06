# Changelog

本專案所有重要變更都記錄在這個檔案。

格式遵循 [Keep a Changelog](https://keepachangelog.com/zh-TW/1.1.0/)，
版號採 [Semantic Versioning](https://semver.org/lang/zh-TW/spec/v2.0.0.html)：
**MAJOR.MINOR.PATCH** = 不相容變更 / 新功能 / 修正。

## [Unreleased]

### Security

接續 v1.1.1 修補剩餘 Medium / Low 等級問題：

- **MED** TOCTOU race：cache refresh 加 `mkdir` atomic lock，避免多個 prompt 並發時重複打 API + 寫入競態。lock 超過 30s 視為前一輪 crash 自動清除
- **MED** `git diff --numstat` 在大型 repo 上無上限，可被當 prompt-render DoS 觸發。加 `timeout 3` + `head -n 200` 雙重界限
- **LOW** `COLUMNS` 未驗證為正整數，非數字值（如 `COLUMNS=wide`）會讓 `[ -gt ... ]` 噴錯到 stderr。改用 regex `^[1-9][0-9]*$` 嚴格驗證
- **LOW** macOS `security find-generic-password` 沒有 timeout，keychain 鎖死時會無限阻塞 prompt。加 `timeout 3`，與 `secret-tool` 一致
- 修正 version cache 在 GitHub API 回 404／rate-limit 時不會被快取的問題（每次 prompt 都重打 API ~5s）。改成只要 response 非空就 cache，`tag_name` 抽取容忍缺欄位

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

[Unreleased]: https://github.com/gn00678465/StatusLine/compare/v1.1.1...HEAD
[v1.1.1]: https://github.com/gn00678465/StatusLine/compare/v1.1.0...v1.1.1
[v1.1.0]: https://github.com/gn00678465/StatusLine/compare/v1.0.0...v1.1.0
[v1.0.0]: https://github.com/gn00678465/StatusLine/releases/tag/v1.0.0
