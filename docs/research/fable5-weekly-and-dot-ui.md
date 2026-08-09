# Fable 5 weekly usage 與 dot UI 研究

## 範圍與版本

本文件只使用以下兩個 primary-source repository 的原始碼、README、圖片與 Git metadata：

- `claude-code-status-line`：HEAD `a2cfda1b135d925b064922c6524c5e779ef9252d`（版本化腳本為 `5.5.0`）；Fable weekly 功能由 commit `1e0fd5f555775b5445579ea10a72469ba31f0995` 引入。來源：`/d/Projects/status-line-dev/claude-code-status-line/claude-code-status-line.py:38`，以及該 repository 的 `git log` / `git show 1e0fd5f`。
- `claude-statusline`：HEAD `ea02c0e6dcd532fea6056f7eec2b7545b3666248`（`main` / `origin/main`）。來源：該 repository 的 `git rev-parse HEAD` 與 `git log -1`。

兩個 clone 都因換行格式而顯示 modified，但針對本研究引用的檔案執行 `git diff --ignore-space-at-eol --exit-code` 均為 0；下列行號內容因此與各自 HEAD blob 語意相同。

## 結論摘要

1. Fable 5 的獨立 weekly 額度不在 Claude Code stdin 的 `rate_limits`，也不是 OAuth response 的固定 `.seven_day_fable` 欄位。腳本額外呼叫 legacy OAuth usage endpoint，從 top-level `limits[]` 尋找 `kind == "weekly_scoped"` 且 `scope.model.display_name` 等於設定模型名稱（預設 `Fable`）的 entry，再讀取 `percent` 與 `resets_at`。來源：`/d/Projects/status-line-dev/claude-code-status-line/claude-code-status-line.py:654-676`、`:2418-2435`；README `/d/Projects/status-line-dev/claude-code-status-line/README.md:391-401`。
2. OAuth request 是 `GET https://api.anthropic.com/api/oauth/usage`，使用 Claude Code OAuth access token 之 Bearer auth、`anthropic-beta: oauth-2025-04-20`，並以 5 秒 timeout 執行 `curl -s -f`。來源：`/d/Projects/status-line-dev/claude-code-status-line/claude-code-status-line.py:733-767`。
3. `claude-statusline` 的版本化程式實際是 **10 顆 dot，不是 11 顆**：`bar_width=10`。README 連結的 demo 圖視覺上是 11 顆，但它與從 initial commit 起一直維持 10 顆的可執行程式不一致。產品實作若明確要求 10 顆，應依程式碼而不是 demo 圖。來源：`/d/Projects/status-line-dev/claude-statusline/bin/statusline.sh:290-312`；README `/d/Projects/status-line-dev/claude-statusline/README.md:3-5`；圖片 `/d/Projects/status-line-dev/claude-statusline/.github/demo.png`；Git commits `afcc63d05cc93eccceec716d1f7400cedfb9ab26`、`0c2b875ec2482dc6647c2f8703c3f5dc2a91556e`。
4. Dot UI 不是每顆 dot 各自使用不同警戒色。整列依「總使用百分比」選一種顏色：0–49 綠、50–69 橘、70–89 黃、≥90 紅；filled 是 `●`，empty 是同色但套用 dim 的 `○`。來源：`/d/Projects/status-line-dev/claude-statusline/bin/statusline.sh:11-21`、`:26-50`。

## 1. Fable 5 weekly 額度資料流

### 1.1 為何必須走 OAuth API

Claude Code 2.1.80+ 的 stdin `rate_limits` 只正規化 `five_hour` 與 `seven_day`：stdin fields 是 `used_percentage` 和 Unix timestamp `resets_at`，內部 fields 則改成 `utilization` 與 ISO-8601 `resets_at`。正規化迴圈並沒有任何 per-model key。來源：`/d/Projects/status-line-dev/claude-code-status-line/claude-code-status-line.py:618-640`。

因此，即使 stdin 已提供一般 5-hour / 7-day 額度，只要 `usage_fable` segment 啟用，主流程仍會額外取得 OAuth response。若 `only_current=1`，只有 active model label 包含設定模型名稱時才 fetch；active-model 比對是 case-insensitive substring，因此 `Fable 5 (1M context)` 會匹配 `Fable`。來源：`/d/Projects/status-line-dev/claude-code-status-line/claude-code-status-line.py:643-651`、`:2418-2432`。

此資料路徑已被 source repository 標為 deprecated：若 legacy OAuth endpoint 被移除，而且 stdin 仍沒有 per-model limits，Fable segment 將停止顯示。來源：`/d/Projects/status-line-dev/claude-code-status-line/claude-code-status-line.py:716-721`；README `/d/Projects/status-line-dev/claude-code-status-line/README.md:394-395`。

### 1.2 Credential 與 authentication

OAuth access token 的查找順序是：

1. macOS 先執行 `security find-generic-password -s "Claude Code-credentials" -w`，將回傳 JSON 的 `.claudeAiOauth.accessToken` 取出；timeout 是 5 秒。來源：`/d/Projects/status-line-dev/claude-code-status-line/claude-code-status-line.py:680-705`。
2. 非 macOS，或 Keychain 失敗時，讀取 `~/.claude/.credentials.json` 的同一 `.claudeAiOauth.accessToken`。來源：`/d/Projects/status-line-dev/claude-code-status-line/claude-code-status-line.py:612-615`、`:707-713`。
3. 此腳本沒有把環境變數列為 OAuth token source。取得 token 後只接受英數字與 `-._~+/=`；空值或其他字元會直接停止 request。來源：`/d/Projects/status-line-dev/claude-code-status-line/claude-code-status-line.py:733-738`。

Request 的精確設定：

```text
GET https://api.anthropic.com/api/oauth/usage
Accept: application/json
Content-Type: application/json
User-Agent: claude-code/2.0.32
anthropic-beta: oauth-2025-04-20
Authorization: Bearer <token>
```

程式用 `curl -s -f --config -`，並從 subprocess stdin 傳入 Authorization header，避免 token 出現在 process list；timeout 為 5 秒。來源：`/d/Projects/status-line-dev/claude-code-status-line/claude-code-status-line.py:740-763`。

### 1.3 OAuth response fields 與 Fable entry 選取

腳本實際依賴的 per-model response shape 是：

```json
{
  "limits": [
    {
      "kind": "weekly_scoped",
      "scope": {
        "model": {
          "display_name": "Fable"
        }
      },
      "percent": 55,
      "resets_at": "<ISO-8601 timestamp>"
    }
  ]
}
```

這是依程式的 field access 所得之最小 shape，不代表完整 API schema。選取規則如下：

- 走訪 `data["limits"]`；非 object 或 `kind != "weekly_scoped"` 的 entry 被略過。
- 取 `scope.model.display_name`，trim 後與設定的 model 名稱做 **case-insensitive exact match**。這與 active-model 的 substring match 不同。
- matching entry 必須同時有 `percent`（允許 0）和非空 `resets_at`，否則回傳 `None`。
- 成功後轉成內部 `{ "utilization": percent, "resets_at": resets_at }`。主流程再暫存於合併後 usage data 的 synthetic `seven_day_fable` key；API 本身沒有此 flat key。

來源：`/d/Projects/status-line-dev/claude-code-status-line/claude-code-status-line.py:654-677`、`:2429-2435`。README 亦明確說明 dynamic `weekly_scoped` 與 display-name keying：`/d/Projects/status-line-dev/claude-code-status-line/README.md:393-401`。

帳號是否具有 per-model cap，不由腳本推算 plan tier、model access 或 extra-usage 狀態；只有 matching `weekly_scoped` entry 的存在被視為可顯示證據。來源：`/d/Projects/status-line-dev/claude-code-status-line/README.md:391-398`。

### 1.4 Cache、fallback 與 error handling

OAuth response cache 位於 `~/.claude/.usage_cache.json`，內容為 `{ "timestamp": ..., "data": ... }`；TTL 使用 `SL_USAGE_CACHE_DURATION`，預設 300 秒。fresh cache 直接回傳 `data`。來源：`/d/Projects/status-line-dev/claude-code-status-line/claude-code-status-line.py:41-59`、`:612-615`、`:723-731`。

成功解析 API JSON 後，以同一目錄的 temporary file 加 `os.replace` 原子更新 cache。cache 寫入失敗只會清除 temporary file，不會丟棄這次已取得的 API data。來源：`/d/Projects/status-line-dev/claude-code-status-line/claude-code-status-line.py:767-784`。

以下情況皆靜默回傳 `None`：credential 無法取得或 JSON 無效、token 字元不安全、`curl` 非 0（包含 `-f` 的 HTTP error）、5 秒 timeout、response JSON 無效、`curl` 不存在。cache 讀取錯誤或 cache JSON 無效則忽略 cache，繼續嘗試 fetch。來源：`/d/Projects/status-line-dev/claude-code-status-line/claude-code-status-line.py:686-713`、`:723-738`、`:740-786`。

主流程用 `oauth_tried` 記住本次 render 是否已 fetch，避免 stdin 缺失且 OAuth 失敗時為 Fable 再付出第二次 5 秒 timeout。來源：`/d/Projects/status-line-dev/claude-code-status-line/claude-code-status-line.py:2407-2432`；feature commit `1e0fd5f555775b5445579ea10a72469ba31f0995` 的 commit message 也明列此設計。

Fable 特有的 failure behavior 是 self-gating：沒有 matching entry、缺 `percent` / `resets_at`、OAuth offline、或帳號沒有 per-model cap 時，`usage_fable` 都是空字串，而不是錯誤訊息。若整份 usage data 是 `None`，只有 5-hour segment 顯示 `usage: N/A`，weekly 與 Fable 仍是空字串。來源：`/d/Projects/status-line-dev/claude-code-status-line/claude-code-status-line.py:1336-1372`、`:1642-1653`；README `/d/Projects/status-line-dev/claude-code-status-line/README.md:393-401`。

### 1.5 Fable weekly 顯示邏輯

Fable 使用與 shared weekly 相同的 168-hour window 與 `%a %H:%M` local-time reset label。`remaining_pct` 是 `max(0, int(100 - utilization))`，因此顯示的是剩餘百分比，並以 Python `int` 截斷，而非四捨五入。來源：`/d/Projects/status-line-dev/claude-code-status-line/claude-code-status-line.py:1346-1385`。

顏色不是只看已用百分比，而是先計算 forward-looking ratio：

```text
remaining budget % / remaining time %
```

- ratio ≥ 1.333…：light，well ahead
- ratio ≥ 1.0：green，on track
- ratio ≥ 0.75：yellow，faster than sustainable
- ratio < 0.75：red，預計在 reset 前用完

來源：`/d/Projects/status-line-dev/claude-code-status-line/claude-code-status-line.py:1387-1403`、`:1118-1130`；README `/d/Projects/status-line-dev/claude-code-status-line/README.md:308-325`。

Fable 可使用與另外兩個 limit 相同的 `blocks`、`vertical`、`none` gauge styles，預設 `blocks` width 4；使用者亦可設定 `model`、`only_current`、`label=full|short|none`。預設 label 是 `Fable`。來源：`/d/Projects/status-line-dev/claude-code-status-line/claude-code-status-line.py:73-86`、`:1462-1483`；README `/d/Projects/status-line-dev/claude-code-status-line/README.md:118-144`。

兩個差異不可誤套至 Fable：5-hour 的剩餘 ≤5% 強制紅、≤10% 強制黃之 safety override 只檢查 `api_key == "five_hour"`；burndown 只檢查 shared `seven_day`，不處理 synthetic `seven_day_fable`。來源：`/d/Projects/status-line-dev/claude-code-status-line/claude-code-status-line.py:1405-1413`；README `/d/Projects/status-line-dev/claude-code-status-line/README.md:327`、`:399`。

## 2. `claude-statusline` 的 dot UI

### 2.1 實際 dot 數與 demo 圖矛盾

`bin/statusline.sh` 將單一 `bar_width` 固定為 10，並把同一值傳給 current（5-hour）、weekly、extra 三列。因此目前可執行規則是 10 顆。來源：`/d/Projects/status-line-dev/claude-statusline/bin/statusline.sh:290-320`。

README 的 demo 圖 `/d/Projects/status-line-dev/claude-statusline/.github/demo.png` 視覺上則是：

- current 28%：3 顆實心 + 8 顆空心，共 11 顆；
- weekly 79%：9 顆實心 + 2 顆空心，共 11 顆。

README 只嵌入圖片，沒有以文字規範 dot count 或 rounding。來源：`/d/Projects/status-line-dev/claude-statusline/README.md:1-5`。Git history 顯示程式的 `bar_width=10` 自 initial commit `afcc63d05cc93eccceec716d1f7400cedfb9ab26` 已存在，圖片則由該 initial commit 加入、於 `0c2b875ec2482dc6647c2f8703c3f5dc2a91556e` rename 至 `.github/demo.png`；歷史內沒有將程式 width 改成 11 的 commit。故圖片不能作為 11-dot 演算法的可靠規格。

### 2.2 Glyph、顏色與 empty 語意

顏色常數與 threshold 是：

| 整列使用率 | 顏色 | RGB | 語意（由 threshold 推得） |
|---:|---|---|---|
| 0–49% | green | `0,175,80` | 正常 |
| 50–69% | orange | `255,176,85` | 第一層警示 |
| 70–89% | yellow | `230,200,0` | 第二層警示 |
| ≥90% | red | `255,85,85` | 高使用警示 |

來源：`/d/Projects/status-line-dev/claude-statusline/bin/statusline.sh:11-21`、`:26-33`。Source code 沒有為這四區間附上更具體的自然語言名稱，因此上表不延伸推測「安全／危險」以外的產品語意。

Filled glyph 是 `●`，empty glyph 是 `○`。整列先輸出 `bar_color`；輸出 empty 前只追加 ANSI dim，沒有 reset 或換色，因此 empty 是「相同 RGB、dim 的空心圓」，不是無色空格，也不是另一個 threshold 顏色。百分比數值本身再次使用同一 `color_for_pct`。來源：`/d/Projects/status-line-dev/claude-statusline/bin/statusline.sh:35-50`、`:294-312`。

### 2.3 Clamp、rounding 與 filled 計算

`build_bar` 先將 pct clamp 至 0..100，再計算：

```text
filled = floor(pct * width / 100)
empty  = width - filled
```

Bash 對非負整數的 `/` 是整數除法；width 10 時，0–9% 是 0 filled、10–19% 是 1、依此類推，90–99% 是 9，只有 100% 是 10。來源：`/d/Projects/status-line-dev/claude-statusline/bin/statusline.sh:35-48`、`:290-296`。

進入 builder 前，stdin 的 5-hour 值用 shell `printf "%.0f"`，weekly 用 awk `printf "%.0f"`；OAuth fallback 的兩者也用 awk `%.0f`。因此流程是「先格式化至 0 位小數，再以整數除法算 filled」，不是直接拿 raw float 做 dot rounding。對恰好 `.5` 的 tie behavior 不應超出各執行環境 `printf` / awk 的實際結果另作假設。來源：`/d/Projects/status-line-dev/claude-statusline/bin/statusline.sh:191-205`、`:271-277`。

以實際程式計算，demo 的 28% 應是 `2● + 8○`，79% 應是 `7● + 3○`；這再次證明 demo 圖不是現行演算法的精確輸出。

### 2.4 Missing data 與 reset 顯示

stdin 是 5-hour / weekly 的 primary source：讀取 `.rate_limits.five_hour.used_percentage` 與 `.resets_at`，並讀取 `.rate_limits.seven_day.used_percentage` 與 `.resets_at`。只有 5-hour percent 存在時才將整組 stdin rates 視為可用。來源：`/d/Projects/status-line-dev/claude-statusline/bin/statusline.sh:191-205`。

若沒有 stdin rates，腳本使用 OAuth/cache fallback；API fields 是 `.five_hour.utilization`、`.five_hour.resets_at`、`.seven_day.utilization`、`.seven_day.resets_at`。ISO reset 先經 `iso_to_epoch`。來源：`/d/Projects/status-line-dev/claude-statusline/bin/statusline.sh:207-280`、`:79-108`。

- 5-hour percent 非空才輸出 `current` 列；weekly percent 非空才輸出 `weekly` 列。缺列時沒有 `N/A` placeholder。來源：`/d/Projects/status-line-dev/claude-statusline/bin/statusline.sh:294-313`。
- 5-hour reset 使用 `time` style，顯示本地時間（macOS `%l:%M%p` 或 GNU `%l:%M%P`），去前導空格、去句點並轉小寫。來源：`/d/Projects/status-line-dev/claude-statusline/bin/statusline.sh:53-64`、`:295`。
- weekly reset 使用 `datetime` style，顯示本地 `%b %-d, %l:%M%p/P`，整理空格並轉小寫。來源：`/d/Projects/status-line-dev/claude-statusline/bin/statusline.sh:65-69`、`:305`。
- Reset 格式化結果非空時，才追加 dim `⟳` 與 white reset text；reset 缺失或 parse 失敗不會隱藏 usage row，只會省略 reset suffix。來源：`/d/Projects/status-line-dev/claude-statusline/bin/statusline.sh:53-77`、`:300-312`。

### 2.5 `install.js` 不含 dot 規則

`bin/install.js` 只負責將 `bin/statusline.sh` 複製到 `~/.claude/statusline.sh` 並設定 Claude Code command。來源：`/d/Projects/status-line-dev/claude-statusline/bin/install.js:7-10`、`:127-161`。JS 內的 green/yellow/red 是 installer 的 success/warn/fail 訊息顏色，不是 usage dot thresholds。來源：`/d/Projects/status-line-dev/claude-statusline/bin/install.js:12-17`、`:23-33`。

## 3. 對 `claudeStatusLine.sh` 實作的直接含意

以下是由上述 primary sources 可直接支持的邊界，並非已完成的產品實作：

1. Fable weekly 必須以 OAuth response 的 dynamic `limits[]` / `weekly_scoped` / `scope.model.display_name` 取得，不能假設 `.seven_day_fable` API field，也不能只靠 stdin `rate_limits`。
2. 建議將 matching entry 的 `percent` 與 `resets_at` 正規化為和 shared weekly 相同的內部 shape，再重用 weekly 顯示路徑；資料不存在或 fetch 失敗時應靜默不顯示 Fable row。
3. 若產品要求 context、5-hour、weekly、Fable weekly 統一以一個環境變數切換 bar / dots，dot mode 應讓所有這些 meter 共用同一模式選擇，而不是各自設 switch。
4. 需求指定 10 dots 時，可靠參考是 `bar_width=10` 的版本化腳本：`●` / dim `○`、10 顆固定總數、先 clamp、filled 使用 floor。不要複製 README demo 的 11 顆與其不一致的 filled 數。
5. 若要精確複製 `claude-statusline` 的 dot 色彩，顏色依「已使用百分比」的四段 threshold 決定；這與 `claude-code-status-line.py` 的 forward-looking remaining-budget/time ratio 色彩是兩套不同語意，實作時不可混為一談。
