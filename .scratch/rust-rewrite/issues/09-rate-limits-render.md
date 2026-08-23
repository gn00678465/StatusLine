# 09 — Rate limits 渲染

Status: done
Type: task
Blocked by: 06, 08

## 範圍

limits 區塊(spec §4.1-5):

- 內建優先:stdin `rate_limits` 有值時渲染 5h/7d(resets_at epoch → `@HH:MM` / `@Mon D, HH:MM` 本地時間);0% 與缺失要能區分;clamp 100
- 無內建時走 OAuth 的 5h/7d;兩者皆無 → `5h: - · 7d: -`
- `📊` 圖示只掛在第一個 rate-limit 子塊
- weekly_scoped 泛化(D7):不論內建/OAuth 路徑,OAuth 有資料就 append 全部 weekly_scoped(`<display_name>: <meter> <pct>% @<reset>`);extra usage `extra: $u/$l` 配色依 utilization
- 時間格式化:epoch/ISO8601 → 本地時區,自寫轉換(避免拉 chrono;`Clock` 供 offset 注入以利測試)

## 驗收

- [x] snapshot:內建、OAuth、皆無、僅 7d(`status-input-seven-day-only.json`)、多 weekly_scoped、extra 開啟
- [x] 時間格式測試:UTC/本地換算、月名縮寫、跨年

## Comments

- 完成 `render::limits`：內建 rate limits 優先、OAuth fallback、無資料 placeholder，以及 OAuth 的 weekly_scoped/extra 附加區塊；`📊` 僅加在第一個子區塊。
- 以 `LocalOffset` 注入本地時區；實作 epoch 與含小數秒、`Z`／`+00:00` ISO8601 的純 Rust 民用日期轉換，並提供系統 offset 實作。
- 新增 snapshot／單元測試，涵蓋內建、OAuth、多 weekly、extra、僅 7d fixture、0% 與缺失區分、clamp、UTC／固定 offset、月名及跨年。
- 驗證通過：`cargo fmt --check`；`cargo test --all-targets`（63 unit + 2 integration）；`cargo clippy --all-targets -- -D warnings`；`cargo build --release`。
- 偏離：無。
- 驗證(orchestrator, 2026-08-23):內建優先/0%與缺失區分/📊 一次性/皆無 placeholder/weekly+extra 獨立 append 與 shell 751-829 對等;時間格式 %H:%M 補零、%b %-d 日不補零、ISO 清洗(小數秒/+00:00)與 epoch 雙路徑、offset 注入 DST-correct;內建 pct 截斷(%%.*)與 OAuth round 語意各自對等;extra 顏色走 bar 階梯。獨立重跑 63+2 全綠。→ done
