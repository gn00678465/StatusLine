# Changelog

本專案所有重要變更都記錄在這個檔案。

格式遵循 [Keep a Changelog](https://keepachangelog.com/zh-TW/1.1.0/)，
版號採 [Semantic Versioning](https://semver.org/lang/zh-TW/spec/v2.0.0.html)：
**MAJOR.MINOR.PATCH** = 不相容變更 / 新功能 / 修正。

## [Unreleased]

<!-- 在這裡記錄尚未發布的變更，發版時再移到對應版本 section -->

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

[Unreleased]: https://github.com/gn00678465/StatusLine/compare/v1.1.0...HEAD
[v1.1.0]: https://github.com/gn00678465/StatusLine/compare/v1.0.0...v1.1.0
[v1.0.0]: https://github.com/gn00678465/StatusLine/releases/tag/v1.0.0
