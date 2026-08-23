# 08 — OAuth token 與 usage API

Status: review
Type: task
Blocked by: 03

## 範圍

`oauth.rs`(spec §4.3):

- `CredentialStore` trait;真實實作依平台:env `CLAUDE_CODE_OAUTH_TOKEN` → macOS `security`(service 名含 `CLAUDE_CONFIG_DIR` sha256 前 8 碼變體,3s timeout)→ `.credentials.json` → Linux `secret-tool`(2s timeout);Windows 僅 env + 檔案
- `HttpClient` trait;真實實作 `ureq`(rustls):GET `api.anthropic.com/api/oauth/usage`,headers 對等 shell 版(`anthropic-beta: oauth-2025-04-20`、UA `claude-code/2.1.34`),connect 3s / total 10s;token 絕不出現在 argv 或子行程命令列
- 60 秒檔案快取 + mkdir 鎖(30s stale);回應含 `five_hour` 才寫入
- 回應解析 structs:`five_hour`/`seven_day`(utilization、resets_at ISO8601)、`extra_usage`、`limits[]` 全部 `weekly_scoped` 條目(D7:不過濾模型名,保留 display_name + percent + resets_at)

## 驗收

- [x] token 來源鏈測試(mock store):各來源優先序、全空 → 不發請求
- [x] 快取測試:60s 內不重打、鎖被占時用 stale、回應無 five_hour 不污染快取
- [x] 解析測試:`fixtures/status-input-oauth.json` 情境 + 0/1/多 weekly_scoped + extra 開/關

## Comments

- 新增 `CredentialStore` / `HttpClient` seams 與 `OAuthUsageFetcher`：mock 驗證 env、keychain、credentials file、secret-tool 的優先序，以及所有來源都空時完全不發 HTTP 請求。真實憑證 adapter 依平台執行 env → macOS `security`（自訂 `CLAUDE_CONFIG_DIR` 用 sha256 前 8 碼 service 名，3 秒 thread+mpsc timeout）→ `.credentials.json` → Linux `secret-tool`（2 秒）；Windows 在編譯期僅保留 env + 檔案。
- 新增 rustls `ureq` adapter，GET OAuth usage 並帶齊 Authorization、Accept、Content-Type、anthropic-beta 與 User-Agent headers，connect timeout 3 秒、總 timeout 10 秒；token 僅留在 HTTP header/子行程 stdout 解析中，不進 argv。
- 實作 shell 對等的 `usage-cache-<sanitized-config-dir>.json` 原始 JSON 快取（60 秒）與 30 秒 stale mkdir lock：fresh hit 跳過 HTTP、鎖被占時回傳 stale、只有成功解析且有 `five_hour` 的回應才原子寫入。另加入 `sha2`（核准用途：macOS keychain service hash）及 `ureq`（核准用途：rustls HTTP）兩個 production dependencies。
- `parse_usage` 保留 five/seven day、extra usage 與所有 `weekly_scoped` 條目（不依模型名稱篩選）；百分比採 shell 相同四捨五入，輸出用字串會過濾控制/BiDi/zero-width 字元。測試讀取 `fixtures/status-input-oauth.json` 的無內建額度情境，並涵蓋 0/1/多條 scoped weekly 與 extra 開關。
- TDD 證據：先為 env 優先的 fetch seam 建立缺少 trait/fetcher 的 RED，實作最小憑證/HTTP 切片轉 GREEN；再以 60 秒第二次請求 RED 驅動快取/鎖，並以不存在的 `parse_usage` RED 驅動完整 response model 與泛化 weekly 解析。
- 本機驗證成功：`cargo fmt --check`（exit 0）；`cargo test --all-targets`（56 unit + 2 integration 全過）；`cargo clippy --all-targets -- -D warnings`（exit 0）；`cargo build --release`（exit 0）。
- 偏離：無。
