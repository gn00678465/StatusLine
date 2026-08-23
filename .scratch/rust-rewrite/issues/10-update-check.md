# 10 — 更新檢查

Status: review
Type: task
Blocked by: 03, 08

## 範圍

`update.rs`(spec §4.1-7):

- GitHub `repos/gn00678465/StatusLine/releases/latest`,24h 檔案快取(非空回應一律快取,含 404/rate-limit JSON),mkdir 鎖
- `tag_name` 清洗(僅 `[a-zA-Z0-9.+-]`)後 semver 比較 `CARGO_PKG_VERSION`
- 有新版 → 附加行 `Update available: <tag> → <repo URL>`(dim)
- 復用 08 的 `HttpClient` trait

## 驗收

- [x] semver 比較測試(含 v 前綴、缺位、相等、降版)
- [x] 快取行為測試:24h 內不重打、惡意 tag_name 清洗

## Comments

- 新增 `UpdateChecker::check()`：以安全 `CacheDir` 實作 `statusline-version-cache.json` 的 24 小時快取、30 秒 mkdir 鎖與 stale fallback；所有非空 GitHub 回應（包括 rate-limit／404 類錯誤 JSON）都會寫入快取，空回應則不寫。
- 擴充既有 `HttpClient` 的 `get_url` adapter；`ureq` 對 GitHub 請求只帶 `Accept: application/vnd.github+json` 與既有 User-Agent，connect timeout 3 秒、總 timeout 5 秒，沒有 Authorization header。
- 實作 tag_name 白名單清洗、v 前綴／缺位補零的前三段版本比較，以及含 dim ANSI 的更新附加行。
- TDD 證據：先以不存在的 `is_newer_version` 建立版本比較 RED；再以缺少 `UpdateChecker`／`HttpClient::get_url` 的快取測試 RED，完成最小 checker 與 HTTP seam 後轉 GREEN。
- 驗證通過：`cargo fmt --check`；`cargo test --all-targets`（68 unit + 2 integration）；`cargo clippy --all-targets -- -D warnings`；`cargo build --release`。
- 偏離：無。
