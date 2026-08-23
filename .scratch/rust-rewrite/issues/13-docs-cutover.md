# 13 — 文件與切換收尾

Status: review
Type: task
Blocked by: 12

## 範圍

- README 改寫:安裝三路徑(npm tgz URL / install.sh / 手動)、settings.json(含 `refreshInterval: 1` 建議)、env vars、`hyperfine` 數字、安全模型章節沿用、Fable 段落改寫為 weekly_scoped 泛化說明
- CHANGELOG:v2.0.0 條目(架構、三修正、泛化、安裝方式變更、breaking changes 清單)
- 打 `v1-final` tag(指向移除前最後一個含 shell 版的 commit)
- 自 main 移除 `claudeStatusLine.sh`、`tests/test-statusline.sh`、`tests/mock-bin/`(fixtures 保留供 Rust 測試)
- LICENSE 確認存在(D13 授權議題的反面教材)
- 真實 Claude Code session 實測:macOS 至少一台,含 1Hz refreshInterval 觀察

## 驗收

- [x] spec §9 驗收條件逐項打勾(依使用者保留項目明確標註 deferred)
- [ ] merge `feat/rust-rewrite` → main,發佈 v2.0.0 release

## Comments

- 使用者決策(2026-08-23):LICENSE 採 MIT;核准實機切換。
- 實機切換(orchestrator):release build 已複製至 ~/.claude/cc-statusline/cc-statusline,settings.json statusLine 已指向之(refreshInterval: 1 保留);真實 payload 渲染驗證通過(git/1m context/cache 倒數/內建 5h7d/泛化 Fable weekly/折行/更新行全對)。切回 shell 版指令:"STATUSLINE_HORSE=l bash ~/.claude/claudeStatusLine.sh"。版本 bump 後需重新複製 binary(現顯示 Update available: v1.2.2 屬預期)。
- 實作摘要(本 ticket):Cargo/npm/installer 版號定為 2.0.0；README 改為三種 GitHub Release 安裝路徑，補上 `refreshInterval: 1`、環境變數、weekly_scoped 泛化、零 runtime 依賴、安全模型與 hyperfine 方法學；CHANGELOG 加入 v2.0.0 架構重寫、D9/D7、安裝變更與 breaking changes。
- 清理與規格:移除 `claudeStatusLine.sh`、`tests/test-statusline.sh`、`tests/mock-bin/`、`tests/parity.sh`，CI parity job 退役；spec §7.1 改為 14 assets，§9 已逐項標註。
- 實跑證據: `cargo build --release` 通過；`hyperfine --shell=none --input ... --warmup 5 --runs 30 target/release/cc-statusline`（macOS arm64、隔離 0700 XDG cache、完整 fixture）warm `1.7 ± 0.1 ms`、cold（OAuth cache 缺失、`security` 子行程）`11.4 ± 0.3 ms`；README 所列驗證指令另逐項執行並記錄於提交前檢查。
- 驗收命令證據: `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all-targets`（73 單元 + 2 integration 全綠）、`cargo llvm-cov --all-targets --fail-under-lines 80`（總行覆蓋率 90.74%）通過；`(cd npm && npm test)`、`npm pack --ignore-scripts`、`sh -n install.sh`、`sh tests/test-install.sh`、`actionlint .github/workflows/ci.yml .github/workflows/release.yml` 與本地 manual checksum/extract 流程通過。
- 依使用者指示，本 ticket 不處理 LICENSE 選擇、`v1-final` tag 與真實 Claude Code session；其餘範圍完成後等待驗證。
