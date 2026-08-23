# 02 — stdin 輸入解析

Status: review
Type: task
Blocked by: 01

## 範圍

`input.rs` + `config.rs`(spec §3.1、§3.3):

- serde structs:全欄位 `Option` + `#[serde(default)]`,未知欄位忽略,數字型別不符 → 預設(自訂 deserializer 或 `serde_json::Value` 中轉)
- `safe_str` 對等:過濾控制字元(U+0000–001F、007F)、zero-width(200B–200F)、BiDi(202A–202E、2066–2069)、FEFF
- `context_window_size` 缺失/0 → 200000;絕不硬編分母(1m payload 實測案例為 fixture)
- env 解析:`STATUSLINE_USAGE_STYLE`、`STATUSLINE_GIT_CACHE_TTL`(clamp 0–60)、`COLUMNS`(非正整數 → 100)

## 驗收

- [x] 單元測試:惡意字串(ESC 注入、BiDi)、型別錯置(數字欄位給字串/物件)、空物件、`effort` 缺失
- [x] 5 份既有 fixtures 都能解析出正確欄位值

## Comments

- 完成 `input.rs` 的 serde `Option`/`#[serde(default)]` schema、數值欄位的 lossy `serde_json::Value` deserializer，以及 model、effort、cwd、session 的 `safe_str` 過濾。缺失、零值或型別錯置的 context window size 會回落 200,000；1,000,000 payload 值會原樣保留。
- 完成 `config.rs` 的 `STATUSLINE_USAGE_STYLE`、`STATUSLINE_GIT_CACHE_TTL`（0–60 clamp，預設 2）與 `COLUMNS`（非正整數預設 100）解析。
- TDD 證據：先後執行 sanitization、context size 型別錯置、5 fixtures、config 的目標測試，均先因缺少 `parse`/存取器/設定型別而 RED；各最小實作後皆 GREEN。
- 本機驗證成功：`cargo fmt --check`、`cargo test --all-targets`（9 unit + 2 integration 全過）、`cargo clippy --all-targets -- -D warnings`、`cargo build --release`。
- 依核准的微小超範圍項，已把兩個 CI job 的 `actions/checkout` 由 v4 升至 v5，以消除 Node 20 淘汰警告。
- 偏離：無。
