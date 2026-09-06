# Changelog

本專案所有重要變更都記錄在這個檔案。

格式遵循 [Keep a Changelog](https://keepachangelog.com/zh-TW/1.1.0)，版號採
[Semantic Versioning](https://semver.org/lang/zh-TW/spec/v2.0.0.html)。

## [Unreleased]

## [v2.1.0] - 2026-09-06

### Added

- 新增使用者設定檔 `~/.config/cc-statusline/config.toml`（尊重
  `XDG_CONFIG_HOME`），可設定 `usage_style`／`git_cache_ttl`。優先序逐鍵決
  定：非空環境變數 > 設定檔 > 內建預設。binary 永不建立或寫入該檔案；設定
  檔不存在時靜默套用預設，格式錯誤或無法讀取時整份忽略並於 stderr 印出一行
  `cc-statusline: ignoring config file <path>: <error>`，stdout 與 exit code
  不受影響。檔案上限 16 KiB，超過即視為無法讀取（整份忽略＋stderr 提示）；
  TOML 解析在專用的 16 MiB stack thread 上執行，避免深度巢狀內容造成 stack
  overflow。

### Notes

- 無 breaking change：環境變數行為與沒有設定檔時的輸出完全不變，直接更新
  binary 即可。
- chezmoi 使用者可在 source 加入 `dot_config/cc-statusline/config.toml`（例如
  `usage_style = "dots"`），設定不再受 `~/.claude/settings.json` 重新套用影響。
- Cargo、npm installer 與 POSIX installer 版本同步至 `2.1.0`。（#10）

## [v2.0.1] - 2026-08-23

### Documentation

- 將 `install.sh` 提升為 README 首選安裝方式，並補充 npm 12 的
  `allow-remote=none` / install-script 預設封鎖行為。
- npm tarball 指令改用完整 `releases/latest/download` URL 搭配
  `--allow-remote=all` 與 `--allow-scripts`；說明 npm 對 tarball 的套件名建議
  無法匹配，舊版 npm 則會靜默忽略旗標。

### Fixed

- postinstall 被 `--ignore-scripts` 或 npm script policy 跳過/封鎖時，診斷改提供
  完整雙旗標 npm 指令與 `install.sh` 替代方案。
- Cargo、npm installer 與 POSIX installer 版本同步至 `2.0.1`，避免 binary
  更新檢查與安裝器發佈版本不一致。

## [v2.0.0] - 2026-08-23

### Changed

- 以單一 Rust `cc-statusline` binary 重寫整個 render 路徑；六個 release
  target、OAuth/Git/時計 trait 注入、atomic cache 與 fail-closed 目錄驗證
  一併定版。
- D9 三項行為修正正式納入：stdin `effort.level`/`xhigh`、官方
  `context_window.used_percentage` 優先、所有 timeout 改為 Rust thread +
  channel，移除 GNU `timeout` 依賴。
- D7 將原本的 Fable 特例改為泛化的 `limits[]`：所有
  `kind == "weekly_scoped"` 條目依 API 回應順序保留。
- 發佈方式改為 GitHub Release assets，提供 npm tgz、校驗過的 POSIX
  `install.sh` 與手動下載；npm 只負責安裝原生 binary。

### Breaking changes

- 安裝路徑由 `~/.claude/claudeStatusLine.sh` 改為
  `~/.claude/cc-statusline/cc-statusline`（Windows 為 `.exe`）。
- `settings.json` 的 `statusLine.command` 必須指向新 binary；建議加入
  `refreshInterval: 1`。設定介面維持環境變數，不再讀取 shell 腳本或 jq
  設定流程。
- 舊 shell 腳本、bash parity harness 與其 mock 資產自主分支移除；更新
  時請重新執行任一安裝方式。`git` 僅是可選的 workspace 增強，不再是
  runtime 的硬性依賴。

### Security

- 快取目錄鏈逐層檢查 ownership、mode bits 與 symlink，目錄建立時即為
  `0700`；所有寫入採 private temp + atomic rename。
- token 不進任何子行程 argv；外部字串與 release tag 在輸出前清洗。

### Migration

1. 安裝 `v2.0.0` binary（npm、`install.sh` 或手動下載）。
2. 將 `settings.json` 的 command 改成新的安裝路徑，並建議設定
   `refreshInterval: 1`。
3. 若需要 Git 分支/狀態，確認執行環境的 `git` 在 `PATH`；其餘 render
   功能不依賴外部指令。

## [v1.2.2] - 2026-08-09

### Changed

- **Usage meter dot spacing**：參考 `claude-statusline`，將 dots 模式的十個
  `●○` 位置直接相連；bar 模式維持連續 `▓░` 色塊。

## [v1.2.1] - 2026-08-09

### Changed

- **Usage meter spacing**：dot 模式的 10 個 `●` / `○` 位置各以一個空格分隔；bar
  模式維持連續 `▓░` 色塊。
- **Semantic emoji**：既有布局加入 `🤖` model、`🧠` effort、`⚡️` context 與單一
  `📊` rate-limit group；原有 `📁` workspace、`🌿` Git 保持不變。
- **Git staged/working tree 語意**：以單次 porcelain v2 snapshot 取代 branch +
  `diff --numstat`；`S` / `W` / `C` 顯示 staged、unstaged、conflict 檔案數。

### Performance

- **Git refresh 降載**：使用 `--no-optional-locks`，略過 untracked、submodule
  dirty 與 rename similarity 掃描，並加入短效 cache。

## [v1.2.0] - 2026-08-09

### Added

- **Fable 5 weekly usage**：從 Anthropic OAuth usage response 的 dynamic
  `limits[]` 取得原始 weekly scope（v2 改以 `weekly_scoped` 泛化渲染）。
- **統一 10-dot usage UI**：設定 `STATUSLINE_USAGE_STYLE=dots` 即可切換 context、5h、
  7d 與 weekly meters。

### Notes

- 舊版 shell 安裝方式與 Fable 專用顯示僅保留在歷史記錄；請依 v2.0.0 migration
  更新設定。

## [v1.1.3] - 2026-05-30

### Fixed

- 修正 macOS 內建 bash 3.2 不支援 `mapfile` 的相容性問題。

## [v1.1.2] - 2026-05-06

### Security

- 完成 cache 目錄 ownership/mode/symlink 驗證、atomic write、字串清洗、token
  不進 argv、timeout 與 mkdir lock 等多輪安全修補；extended ACL 限制沿用 v2
  threat model 說明。

## [v1.1.1] - 2026-05-06

### Security

- OAuth bearer token 改由 stdin/config 讀取，避免出現在 process argv；補強數值與
  terminal escape 清洗。

## [v1.1.0] - 2026-05-06

### Added

- Cache 命中率與 1 小時 TTL 倒數、Git 快取與低負載 porcelain 查詢。

## [v1.0.0] - 2026-04-30

### Added

- 初始 status line 實作：workspace、Git、model、effort、context、5h/7d/extra
  rate limits、折行與更新檢查。

[Unreleased]: https://github.com/gn00678465/StatusLine/compare/v2.1.0...HEAD
[v2.1.0]: https://github.com/gn00678465/StatusLine/compare/v2.0.1...v2.1.0
[v2.0.1]: https://github.com/gn00678465/StatusLine/compare/v2.0.0...v2.0.1
[v2.0.0]: https://github.com/gn00678465/StatusLine/compare/v1.2.2...v2.0.0
[v1.2.2]: https://github.com/gn00678465/StatusLine/compare/v1.2.1...v1.2.2
[v1.2.1]: https://github.com/gn00678465/StatusLine/compare/v1.2.0...v1.2.1
[v1.2.0]: https://github.com/gn00678465/StatusLine/compare/v1.1.3...v1.2.0
[v1.1.3]: https://github.com/gn00678465/StatusLine/compare/v1.1.2...v1.1.3
[v1.1.2]: https://github.com/gn00678465/StatusLine/compare/v1.1.1...v1.1.2
[v1.1.1]: https://github.com/gn00678465/StatusLine/compare/v1.1.0...v1.1.1
[v1.1.0]: https://github.com/gn00678465/StatusLine/compare/v1.0.0...v1.1.0
[v1.0.0]: https://github.com/gn00678465/StatusLine/releases/tag/v1.0.0
