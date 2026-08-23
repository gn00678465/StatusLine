# 12 — 發佈工程:npm 套件、install.sh、release CI

Status: review
Type: task
Blocked by: 11

## 範圍

- `npm/`:`package.json`(`@gn00678465/cc-statusline`,bin 無 — 純安裝器)+ `postinstall.js`:platform/arch 偵測(darwin-arm64/x64、linux-x64/arm64 musl、win32-x64/arm64)→ 從同版 release 下載 asset + `.sha256` → 驗證 → 解壓至 `~/.claude/cc-statusline/`(Windows 為 `%USERPROFILE%\.claude\cc-statusline\cc-statusline.exe`);`--ignore-scripts`/下載失敗給明確錯誤與手動安裝指引
- `install.sh`:同邏輯 POSIX sh 版(uname 偵測、curl/wget 擇一、sha256sum/shasum 擇一);本身也上傳為 release asset
- `release.yml`:tag `v*` 觸發 — 6 target 編譯(darwin 原生 runner、musl 用 cargo-zigbuild、windows 原生 runner)→ tar.gz/zip + per-asset `.sha256` → `npm pack` 產 `cc-statusline-npm.tgz`(檔名不含版本)→ 上傳 §7.1 全部 13 個 assets 並建立 Release
- postinstall 與 install.sh 各附最小測試(mock 下載源)

## 驗收

- [ ] release dry-run(workflow_dispatch)產出全部 assets,命名符合 spec §7.1（待推送後遠端驗證）
- [x] macOS 實機:兩條安裝路徑都完成安裝且 binary 可執行
- [x] 竄改 binary 後 hash 驗證確實失敗且不落地

## Comments

- 新增純安裝器 npm 套件（`@gn00678465/cc-statusline@2.0.0-rc`）：無 `bin`／`private`，封包只含 `package.json` 與 `postinstall.js`。Node 安裝器支援六組 platform/arch、HTTP(S) mock base URL、SHA-256 sidecar 驗證、tar.gz 與 ZIP 解壓、Windows `.exe` 目的地、下載與 `--ignore-scripts` 的手動安裝指引。
- 新增 POSIX `install.sh`：以 `uname`、curl/wget、sha256sum/shasum fallback 安裝；僅在驗證成功後寫入 `~/.claude/cc-statusline/cc-statusline`。
- 新增安裝器測試與三 OS CI job。`npm test`（HTTP mock：正常 tar/ZIP、竄改拒絕、ignore-scripts 診斷）及 `sh tests/test-install.sh`（file://：正常、竄改拒絕）皆在本機 macOS 通過；兩個成功案例均驗證安裝檔具 executable 權限。
- 驗證指令：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all-targets`（75 passed）、`npm test`、`sh tests/test-install.sh`、`shellcheck install.sh tests/test-install.sh`、`actionlint .github/workflows/ci.yml .github/workflows/release.yml`、`npm pack --ignore-scripts --pack-destination <tmp> ./npm` 全數通過。另以 `cargo build --release` 實際封裝 Darwin arm64 tarball，內容為單一 `cc-statusline`，sidecar 格式為 `<hash>  <filename>`。
- 待遠端驗證：尚未推送 workflow，故未啟動 GitHub `workflow_dispatch` 六目標 dry-run；workflow 已設定手動觸發只 build/package/upload artifacts、不建立 Release。
- 偏離／規格衝突：§7.1 列出的 6 archives + 6 sidecars + npm tgz 為 13 個，但 ticket 同時要求把 `install.sh` 也上傳。workflow 依較具體的 install.sh 要求產出 14 個 assets，並在 release job 明確驗證數量。
