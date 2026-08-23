# 06 — 渲染引擎:基礎區塊

Status: done
Type: task
Blocked by: 02, 04, 05

## 範圍

`render/`(spec §4.1-1~3):

- `color.rs`:ANSI truecolor 常數(對等 shell 版 12 色)+ dim/reset
- `meter.rs`:bar `▓░` / dots `●○`(dim 空點)10 格;配色階梯兩套(bar: 90紅/70橘/50黃/綠;dots: 90紅/70黃/50橘/綠)+ pct clamp
- `blocks.rs`:workspace(📁 dir › 🌿 branch [S|W|C])、model & effort(🤖 · 🧠,xhigh=橘、缺失隱藏)、context(⚡️ used/total meter,k/m 格式化捨入對等 shell 版 `format_tokens`,百分比優先用官方 `used_percentage`)
- `mod.rs`:區塊組裝、`│`/`·`/`›` 分隔、與 width 模組銜接折行

## 驗收

- [x] 單元測試:`format_tokens`(999/1000/1500/999999/1000000/1050000 等捨入邊界)、meter 兩樣式全配色段、effort 五等級 + 缺失
- [x] snapshot:workspace+model+context 組合(git 有/無、effort 有/無)

## Comments

- 完成 renderer 的單一 `render(RenderContext)` seam：組裝 workspace、model/effort 與 context，使用 Ticket 05 的 ANSI-aware `display_width` 決定單行或 dim `└─` 的第二行；`│`、`·`、`›` 均以 dim/reset 包裝。
- `color.rs` 完整對等 shell 的 9 個 truecolor、`dim`/`dim_off`/`reset` 共 12 常數；`meter.rs` 實作 10 格 bar/dots、dots 空點 dim、0–100 clamp，以及 bar `90紅/70橘/50黃`、dots `90紅/70黃/50橘` 的刻意互換階梯。
- `blocks.rs` 完成 Windows/Unix workspace 尾目錄、Git branch 與 S/W/C、五級 effort（`medium` 顯示 `med`、`xhigh` 橘色、缺失完全隱藏）、token k/m 進位，以及官方 `used_percentage` 四捨五入優先、缺失才依 token 自算的 context。
- 新增 `insta 1` dev-dependency（spec §5 核准），以 inline snapshots 驗證 Git+effort 與無 Git+無 effort 的完整可見輸出；另有 colors、meter、token、effort、context priority 與折行單元測試。
- TDD 證據：色盤、meter、`format_tokens`、effort/context 與第一個 renderer snapshot 均先 RED（缺少目標 interface/依賴）再逐切片 GREEN；snapshot macro 的 inline 呼叫型別與命名形式經編譯錯誤校正，無行為偏離。
- 本機驗證成功：`cargo fmt --check`、`cargo test --all-targets`（41 unit + 2 integration 全過）、`cargo clippy --all-targets -- -D warnings`、`cargo build --release`。
- 偏離：無。
- 驗證(orchestrator, 2026-08-23):色盤 12 常數 byte-exact;meter 雙階梯(bar 70=橘/dots 70=黃)與 shell 刻意互換一致;format_tokens 進位含 frac carry 邊界全對;effort 五級 + med 縮寫 + 缺失隱藏;context 區塊 ANSI 序列組合順序逐字元對等;官方 used_percentage 優先、自算 fallback floor 對等。獨立重跑 41+2 全綠。→ done
- 小記:mod.rs 的折行判斷與 width::wrap_status_line 有輕微重複,ticket 11 整合時考慮收攏,不影響行為。
