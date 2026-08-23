# 01 — 專案骨架與 CI 基礎

Status: review
Type: task

## 範圍

- `cargo init`(crate 名 `cc-statusline`,edition 2021+),spec §5 的 release profile
- `src/` 模組樹建立(空殼 + doc comment),`main.rs` 讀 stdin → 輸出 `Claude` 的最小可跑版本
- clippy 設定:deny warnings、禁 `unwrap_used`/`expect_used`
- `ci.yml`:fmt + clippy + test(macOS/Linux/Windows)
- shell script 與 bash 測試暫時共存,不動

## 驗收

- [x] `echo '{}' | cargo run` 輸出 `Claude`,exit 0
- [x] 空 stdin 同樣輸出 `Claude`
- [ ] CI 三 OS 全綠（待 GitHub Actions 執行）

## Comments

- 完成 Rust 2021 crate `cc-statusline`、spec §5 release profile、禁止 warnings/`unwrap_used`/`expect_used` 的 clippy 設定，以及含 doc comment 的 `src/` 模組空殼。
- 以 TDD 完成最小 stdin → `Claude` fallback：先執行 `cargo test --test cc_statusline`，確認預設程式的 JSON stdin case 失敗（輸出為 `Hello, world!`）；實作後同一測試通過，並補上空 stdin case。
- 本機驗證皆成功：`cargo fmt --check`、`cargo test --all-targets`（2 passed）、`cargo clippy --all-targets -- -D warnings`、`cargo build --release`、`echo '{}' | cargo run`（輸出 `Claude`，exit 0）、`cargo run < /dev/null`（輸出 `Claude`，exit 0）。
- 已新增 GitHub Actions 三 OS（macOS/Linux/Windows）test matrix 與 fmt/clippy job；尚未推送 branch，因此 CI 綠燈留待遠端驗證。
- 偏離：無。
