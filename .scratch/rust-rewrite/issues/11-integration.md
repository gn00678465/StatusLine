# 11 — 整合組裝與對等驗證

Status: done
Type: task
Blocked by: 07, 09, 10

## 範圍

- `main.rs` 完整組裝:stdin → 各模組(git/OAuth 依序,共用 cache dir)→ render → 折行 → update 行;頂層錯誤攔截(任何失敗 → 至少輸出 `Claude`,exit 0)
- 整合測試:5 份既有 fixtures + 新增 fixtures(xhigh、effort 缺失、1m context、多 weekly_scoped、空 stdin、非 JSON、超寬折行、cache dir 不安全),全 mock 注入,`insta` snapshot
- **對等驗證**:harness 以同一 fixture 分別跑 shell 版與 Rust 版,diff 輸出;僅允許 spec §4.2 三修正 + D7 泛化造成的差異,逐項列入白名單文件
- `cargo-llvm-cov` 接入 CI,80% 門檻生效

## 驗收

- [x] 全 fixtures snapshot 通過;對等 diff 僅含白名單差異
- [x] 覆蓋率 ≥80%
- [x] `hyperfine` 實測(冷/暖快取)數字記錄於 ticket 附註,供 README 引用

## Comments

- 完成 `main.rs` 組裝：輸入失敗回退 `Claude`、session key cwd/no-cwd 回退、Git/TTL/OAuth/limits/render/update 軟降級串接；並將折行判斷統一委派給 `width::wrap_status_line`。
- 新增五份邊界 fixtures，並以全注入 mock 的 insta 整合 snapshot 覆蓋所有十份 fixture、空/非 JSON 輸入與不安全快取目錄。
- `bash tests/parity.sh`：10 份 fixtures 全數通過；時間片段正規化及僅 D9/D7 白名單差異記於 `parity-whitelist.md`。
- `cargo test --all-targets`：75 passed；`cargo fmt --check` 與 `cargo clippy --all-targets -- -D warnings` 通過；`cargo build --release` 通過。
- `cargo llvm-cov --all-targets --fail-under-lines 80`：90.74% lines（75 passed），CI 已加入相同 80% 門檻。
- `hyperfine --warmup 2 --runs 10`（release、版本快取固定以隔離網路）：`{}` 12.3 ms ± 0.3 ms；完整 fixture 冷快取 25.1 ms ± 0.6 ms；完整 fixture 暖快取 12.5 ms ± 0.1 ms。
- 偏離：無；benchmark 的冷快取每輪僅清除暫存目錄內明確的 Git/TTL cache 項目。
- 驗證(orchestrator, 2026-08-23):parity harness 親跑 10 fixtures 全過,白名單(parity-whitelist.md)正規化邊界審核通過;main.rs 組裝與軟降級、session fallback、折行收攏確認;插單修正 Windows set_times 可寫 handle(4969141);完整 CI 六 job 全綠(run 32618145740,含 Coverage 80% 門檻與 Shell parity)。效能覆核:implementor 的 12.3ms 為隔離環境 keychain 子行程成本;orchestrator 於暖快取真實情境實測 2.3ms(vs /bin/echo 0.73ms),符合 ~2ms 目標;README 正式數字於 ticket 13 以雙情境方法學記載。→ done
