# 07 — Cache 命中率與 TTL 狀態機

Status: done
Type: task
Blocked by: 03, 06

## 範圍

`ttl.rs` + cache 區塊渲染(spec §4.1-4):

- 命中率:`round(cache_read * 100 / (input + creation + read))`;≥50% 綠、<50% 灰
- 狀態檔(per-session):`{signature, started_at, last_hit_rate}`;signature = 三 token 數組合,變更才重置 `started_at`;無 usage 且無既存狀態 → 不寫
- 倒數:3600 秒;`MM:SS`;配色 >2400s 綠 / 1200–2400s 黃 / 300–1200s 紅 / ≤300s 依 `now % 2` 閃爍紅/亮紅 / ≤0 `exp` 灰
- 全部經 `Clock` 注入,確定性測試

## 驗收

- [x] 狀態機測試:signature 不變不重置、變更重置、首次 usage 建檔、無 usage 顯示 last_hit_rate
- [x] 配色分段邊界測試(2400/1200/300/0)+ 閃爍奇偶
- [x] snapshot:完整 cache 區塊各狀態

## Comments

- 新增 `CacheTtl::update(TokenUsage)`：透過注入的 `Clock` 與 Ticket 03 的安全 `CacheDir` 讀寫 per-session JSON 狀態。signature 為 `input:creation:read`；首次/變更 usage 才重置 `started_at`，無 usage 且無狀態不建檔，並可單獨沿用既有 `last_hit_rate`。
- 命中率使用整數四捨五入；倒數 3600 秒採 `MM:SS`，完整實作 shell 的 2400/1200/300/到期邊界與偶秒紅、奇秒亮紅。Cache renderer 依 50% 門檻設定綠/灰命中率，並接入 context 後的 dim `·` 分隔與既有 ANSI-aware 折行。
- TDD 證據：先以 `CacheTtl` 的首次建檔狀態測試建立 RED，再實作狀態機；另先讓 renderer snapshot 使用尚未存在的 `cache_status` 欄位，確認編譯 RED 後才完成組裝。測試涵蓋 signature 不變/變更、無 usage 取 last rate（含僅有 last rate 的舊狀態）、空狀態不寫檔、邊界與奇偶閃爍，以及完整 cache block inline snapshots。
- 本機驗證成功：`cargo fmt --check`（exit 0）；`cargo test --all-targets`（47 unit + 2 integration 全過）；`cargo clippy --all-targets -- -D warnings`（exit 0）；`cargo build --release`（exit 0）。
- 偏離：無。
- 驗證(orchestrator, 2026-08-23):hit rate 公式、signature 轉移(含 started_at 缺失重建)、TTL 邊界 <=300/1200/2400、now%2 偶紅奇亮紅、elapsed>=3600→exp、{}:{:02} 時間格式皆與 shell 逐條對等;decode 對殘缺狀態較 shell 寬容(僅 last_hit_rate 亦可用),輸出經 u8 解析安全,核准。獨立重跑 47+2 全綠。→ done
