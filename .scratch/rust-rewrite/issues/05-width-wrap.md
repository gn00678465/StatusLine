# 05 — 顯示寬度與折行

Status: done
Type: task
Blocked by: 01

## 範圍

`width.rs`(spec §4.1-6):

- ANSI escape 序列剝除
- 顯示寬度:`unicode-width` 為基底,補 shell 版 jq 表的差異範圍(emoji U+1F300–1FAFF 計 2、U+26A1 計 2、variation selector FE00–FE0F 計 0、zero-width/BiDi 計 0)——以 shell 版 `terminal_width_inline` 的字元表為對照真值
- 折行決策:`<line1> │ <line2>` 總寬 vs `COLUMNS`;超寬 → 兩行,第二行 `└─ ` 前綴

## 驗收

- [x] 單元測試:`📁⚡️🤖·›▓░●○` 等本 UI 實際使用字元逐一驗寬;混合 CJK;ANSI 剝除
- [x] 折行邊界測試:恰好等寬、差 1、`COLUMNS` 缺失 fallback 100

## Comments

- 完成 `width` 模組：CSI ANSI 剝除、以 `unicode-width` 為基底的顯示寬度計算，以及 `wrap_status_line(out, limit, columns)` 的單行/兩行決策。`unicode-width 0.2` 是 spec §5 核准的必要依賴，提供 Unicode East Asian 寬度基底。
- 依 shell `terminal_width_inline` 對等補正：U+26A1 與 U+1F300–U+1FAFF 為 2 欄；U+0300–036F、zero-width/BiDi 範圍與 U+FE00–FE0F 為 0 欄。ANSI 會在量測前剝除。
- 測試逐一涵蓋 UI glyph `📁 ⚡️ 🤖 🌿 🧠 📊 · › │ └─ ▓ ░ ● ○`、emoji 範圍邊界、全部 variation selectors、CJK 混排與 ANSI；折行驗證 21 欄恰等寬保留單行、20 欄折行，且 `COLUMNS` 缺失回退 100。
- TDD 證據：ANSI strip、UI glyph width 與折行 API 均先 RED（缺少 public function）再以最小實作 GREEN；`COLUMNS` fallback 是既有 config 行為，補上缺失值的精確回歸斷言。
- 本機驗證成功：`cargo fmt --check`、`cargo test --all-targets`（32 unit + 2 integration 全過）、`cargo clippy --all-targets -- -D warnings`、`cargo build --release`。
- 偏離：無。
- 驗證(orchestrator, 2026-08-23):zero-width 五區間與 shell jq 表逐一相符;wide 以 26A1+1F300–1FAFF override 補足,其餘經 unicode-width 覆蓋(Hangul/CJK/fullwidth 抽查一致);CSI 剝除、折行邊界(21/20 欄)、CJK 混排測試皆過。獨立重跑 32+2 全綠。→ done
