# Release Notes 模板

每次發新版時，把 `CHANGELOG.md` 內 `## [Unreleased]` 的內容搬到 `## [vX.Y.Z] - YYYY-MM-DD` 這個新 section，再 push tag `vX.Y.Z`，GitHub Action（`.github/workflows/release.yml`）會自動抽出該段落建立 Release。

## 段落結構

```markdown
## [vX.Y.Z] - YYYY-MM-DD

### Added         <!-- 新功能 -->
### Changed       <!-- 既有功能行為變更 -->
### Fixed         <!-- Bug 修復 -->
### Performance   <!-- 效能優化（非 Keep a Changelog 標準，但常用）-->
### Deprecated    <!-- 即將移除 -->
### Removed       <!-- 已移除 -->
### Security      <!-- 安全性相關 -->
### Documentation <!-- 文件變更 -->
### Notes         <!-- 升級注意事項、breaking change 說明等 -->
```

不適用的 section 直接刪掉，不要留空標題。

## 撰寫規範

- **動詞祈使句**，首字大寫，結尾不加句點：`Add cache hit rate display` 而非 `Added cache...` / `加上快取命中率顯示。`
- **使用者視角**：說明變更帶來的影響，不要堆 commit 訊息
- **Breaking change** 開頭加 `**BREAKING:**`，並在 `### Notes` 寫升級指引
- **連結 issue / PR**：`(#42)` 或 `[#42](url)`，只放確實相關的
- **避免**：客戶名、PII、內部 ticket 原文（用 `Refs INTERNAL-1234` 帶過）

## 範例（從 v1.1.0 摘錄）

```markdown
## [v1.1.0] - 2026-05-06

### Added

- **Cache 命中率 + TTL 倒數區塊**：在 token 使用量之後顯示 `Cache 94% 56:41`
  - 命中率 ≥50% 綠 / <50% 灰
  - TTL 從上次響應起算 1 小時，signature 機制避免每次刷新就重置

### Performance

- 端到端執行時間 ~37s → ~4s（~9x 加速），主要受益於 Windows Git Bash 環境
- 子進程數從 ~50 降到 ~5

### Notes

- 無 breaking change，直接更新腳本即可
```

## 發版流程速查

```bash
# 1. 把 [Unreleased] 內容搬到新版本 section，更新日期
$EDITOR CHANGELOG.md

# 2. 同步 bump 腳本中的 VERSION
$EDITOR claudeStatusLine.sh   # VERSION="1.1.0"

# 3. 更新 CHANGELOG 底部的 compare link
# [Unreleased]: .../compare/vX.Y.Z...HEAD
# [vX.Y.Z]: .../compare/vPREV...vX.Y.Z

# 4. Commit
git add CHANGELOG.md claudeStatusLine.sh
git commit -m "chore(release): vX.Y.Z"

# 5. Tag + push（PR merge 進 main 後在 main 上做）
git checkout main && git pull
git tag -a vX.Y.Z -m "Release vX.Y.Z"
git push origin main
git push origin vX.Y.Z

# 6. GitHub Action 觸發，幾秒後 Release 自動出現
#    手動驗證：https://github.com/gn00678465/StatusLine/releases
```
