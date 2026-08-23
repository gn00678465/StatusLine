# Rust／shell parity whitelist

`tests/parity.sh` 會先去除 ANSI，並只正規化下列核准差異；其餘 diff 一律失敗。

## D9 — 已定行為修正

1. **Effort 來源**：Rust 讀 stdin `effort.level`，shell 讀環境變數／settings。harness 移除兩邊的 `· 🧠 <effort>` 區塊後比較。
2. **官方 context 百分比**：Rust 優先採 `context_window.used_percentage`，shell 自 token 自算。fixture 的官方值目前刻意與自算值相同；此列保留作為核准差異依據。
3. **內建 timeout**：Rust 的 thread+mpsc 取代 shell 的 GNU `timeout`／`gtimeout` 依賴；成功輸出沒有文字差異，因此不需要輸出正規化。

## D7 — 泛化 weekly_scoped

- Rust 依回應順序顯示所有 `weekly_scoped`。shell 僅顯示硬編碼的 Fable。harness 只移除固定 mock 回應中的 `Other` scope，並把 Rust 的原始 `Fable` display name 正規化成 shell 的硬編碼 `Fable 5`；這兩項差異都不會掩蓋其他 scope、百分比或 reset 格式的差異。

## 時間相依欄位

- Cache 倒數與 rate-limit reset 會隨執行時刻／本地時區改變；harness 將其正規化為 `<ttl>`／`<reset>`，只比較其餘輸出。
