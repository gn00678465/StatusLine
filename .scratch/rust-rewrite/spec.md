# Spec: cc-statusline — Rust 重寫 (v2.0.0)

Status: draft
研究依據:[docs/research/rust-nodejs-rewrite.md](../../docs/research/rust-nodejs-rewrite.md)(payload 驗證結果已併入該文件待決事項 #1)

## 1. 目標

將 `claudeStatusLine.sh`(bash,~950 行)重寫為單一 Rust binary `cc-statusline`,達到:

- 視覺輸出與 shell 版完全一致(同圖示、meter、配色、折行行為)
- 每次 render ~2ms(shell 版為數十 ms 的多子行程),以 `refreshInterval: 1`(1Hz)為常態工作負載設計
- 只發佈於 GitHub(Releases + release-attached npm tgz),render 路徑零 Node
- 保留 shell 版的全部安全機制

## 2. 已定決策(grilling 定案,不再重議)

| # | 決策 |
|---|---|
| D1 | 純 Rust binary,無 JS API、無 napi-rs、無 WASM |
| D2 | 名稱:npm 套件 `@gn00678465/cc-statusline`、binary `cc-statusline`(Windows 為 `cc-statusline.exe`)、安裝路徑 `~/.claude/cc-statusline/` |
| D3 | 原地重寫:`feat/rust-rewrite` branch → 合併 → v2.0.0;shell 版打 `v1-final` tag 後自 main 移除 |
| D4 | Windows 原生支援;憑證僅走 `.credentials.json` fallback(Credential Manager 留待後續) |
| D5 | 發佈:GitHub Releases 唯一通道;npm tgz 掛在 release assets 上,`npm i -g <releases/latest/download URL>` 免 PAT 安裝;**不用** npmjs 也**不用** GitHub Packages |
| D6 | `subagentStatusLine` 不進 v2.0.0;CLI 預留子命令空間 |
| D7 | Fable 硬編改為泛化:渲染 OAuth 回應 `limits[]` 中所有 `kind == "weekly_scoped"` 條目 |
| D8 | 設定介面僅環境變數,沿用原名(`STATUSLINE_USAGE_STYLE`、`STATUSLINE_GIT_CACHE_TTL`);設定檔列入 v2 之後 |
| D9 | 隨重寫併入三項已確認修正(見 §4.2),不做 bit-for-bit 對等 |
| D10 | CI targets(6 個,Linux 僅 musl 靜態):`aarch64/x86_64-apple-darwin`、`aarch64/x86_64-unknown-linux-musl`、`aarch64/x86_64-pc-windows-msvc`;musl 走 cargo-zigbuild |
| D11 | Release assets 命名採 Node 風格 + per-asset `.sha256` sidecar(見 §7.1) |
| D12 | 測試:TDD、沿用 `tests/fixtures`、`insta` snapshot、`cargo-llvm-cov` ≥80%、網路/git/憑證以 trait 注入 mock |
| D13 | CCometixLine 血系程式碼一律重新實作,不複製(授權鏈混濁) |
| D14 | OAuth 流程必須保留 — 2026-08-22 payload 實驗證實 stdin `rate_limits` 無 per-model 週額度 |

## 3. 輸入/輸出契約

### 3.1 stdin(Claude Code → binary)

單一 JSON 物件。實測 schema 見研究文件 §1.2 與待決事項 #1 的驗證註記。本專案使用的欄位:

| 欄位 | 用途 | 缺失時 |
|---|---|---|
| `model.display_name` | 模型名 | `"Claude"` |
| `effort.level` | effort 顯示(`low/medium/high/xhigh/max`) | 隱藏 effort 區塊 |
| `cwd` | 目錄名 + git 查詢起點 | 隱藏 workspace 區塊 |
| `session_id` | 快取檔 key | 以 `cwd` 代替 |
| `context_window.context_window_size` | 分母(**payload 值,絕不硬編 200k**;1m session 回報 1000000) | 200000 |
| `context_window.current_usage.{input_tokens,cache_creation_input_tokens,cache_read_input_tokens}` | cache 命中率 + TTL signature | 0 |
| `context_window.used_percentage` | context meter(官方預算值,D9) | 自算 fallback |
| `rate_limits.five_hour.{used_percentage,resets_at}` | 內建 5h(resets_at 為 epoch 秒) | 走 OAuth 或顯示 `-` |
| `rate_limits.seven_day.{used_percentage,resets_at}` | 內建 7d | 同上 |

解析規則(研究 §4.2-3):全欄位 `Option` + `#[serde(default)]`;未知欄位忽略;數字欄位遇型別不符 → 預設值;字串欄位過濾控制字元/BiDi/zero-width(對等 shell 版 `safe_str`);stdin 為空或非 JSON → 輸出 `Claude` 後 exit 0。

### 3.2 stdout(binary → Claude Code)

- 1–2 行(超寬折行)+ 可選的 update 提示行;ANSI truecolor;stderr 永不輸出
- 任何內部錯誤都不得 exit non-zero 或空輸出(官方:non-zero/空輸出 = statusline 消失);最壞情況輸出 `Claude`
- 終端寬度讀 `COLUMNS` env(官方保證,v2.1.153+),非法值 fallback 100

### 3.3 環境變數(輸入)

`STATUSLINE_USAGE_STYLE`(`bar`|`dots`,他值→`bar`)、`STATUSLINE_GIT_CACHE_TTL`(0–60 秒,預設 2)、`CLAUDE_CONFIG_DIR`、`CLAUDE_CODE_OAUTH_TOKEN`、`COLUMNS`。

## 4. 功能需求

### 4.1 區塊(與 shell 版對等)

渲染順序與分隔符(`│` 主分隔、`·` 次分隔、`›` 層級)不變:

1. **Workspace**:`📁 <dir>` + `🌿 <branch> [S<n>|W<n>|C<n>]`。git 查詢:單次 `git --no-optional-locks -C <cwd> status --porcelain=v2 --branch --no-ahead-behind --untracked-files=no --ignore-submodules=dirty --no-renames`,1 秒 timeout,per-session 快取(TTL 見 §3.3,mkdir 原子鎖 + stale fallback 語意對等 shell 版 `_collect_git_status`)
2. **Model & Effort**:`🤖 <model> · 🧠 <effort>`;effort 配色 low=dim / medium=黃(顯示 `med`)/ high=橘 / **xhigh=橘(新)** / max=紅
3. **Context**:`⚡️ <used>/<total> (<meter> <pct>%)`;token 格式化 k/m 捨入規則對等;meter 10 格 bar `▓░` 或 dots `●○`(D8),配色階梯對等(bar: 50/70/90;dots: 50/70/90 但 70 段黃橘互換 — 對等 shell 版現行為)
4. **Cache**:`Cache <hit%> <MM:SS>`;命中率 = cache_read/(input+creation+read) 四捨五入,≥50% 綠否則灰;TTL 3600 秒倒數,signature(三個 token 數的組合)變更才重置;配色 >40m 綠 / 20–40m 黃 / 5–20m 紅 / <5m 依秒數奇偶閃爍紅 / 過期 `exp` 灰;狀態存 JSON(`signature`,`started_at`,`last_hit_rate`)
5. **Rate limits**:內建 5h/7d 優先(`📊` 圖示掛在第一個出現的子塊);無內建時走 OAuth;OAuth 額外供給:**所有 `weekly_scoped` 條目**(D7,標籤用 `scope.model.display_name`,依回應順序)與 `extra: $used/$limit`;百分比 clamp 0–100;reset 時間格式 `@HH:MM`(5h)/`@Mon D, HH:MM`(7d/weekly)
6. **折行**:自算顯示寬度(zero-width/wide/emoji 範圍,`unicode-width` crate + 對等 shell 版 jq 表的 emoji 補充)vs `COLUMNS`;超寬 → 第二行以 `└─ ` 起頭
7. **更新檢查**:GitHub `releases/latest` API,24h 快取,semver 比較 `CARGO_PKG_VERSION`,有新版時輸出附加行

### 4.2 隨重寫併入的行為修正(D9)

1. effort 改讀 stdin `effort.level`(移除 settings.json 讀取與 `CLAUDE_CODE_EFFORT_LEVEL`),支援 `xhigh`,`effort` 欄位缺失時隱藏區塊
2. context 百分比優先用 `context_window.used_percentage` 官方值
3. timeout 全部內建(thread + mpsc),移除對 GNU `timeout`/`gtimeout` 的依賴

### 4.3 OAuth(對等 shell 版)

Token 來源依序:`CLAUDE_CODE_OAUTH_TOKEN` env → macOS keychain(`security find-generic-password`,`CLAUDE_CONFIG_DIR` 時 service 名帶 sha256 前 8 碼,3 秒 timeout)→ `$CLAUDE_CONFIG_DIR/.credentials.json` → Linux `secret-tool`(2 秒 timeout)。Windows 只有 env + 檔案兩源(D4)。

呼叫 `https://api.anthropic.com/api/oauth/usage`(header:`anthropic-beta: oauth-2025-04-20`、User-Agent 硬編 `claude-code/2.1.34`),token 不進 argv/命令列;60 秒檔案快取 + mkdir 鎖(30 秒 stale 清理);回應需含 `five_hour` 才視為有效。

## 5. 非功能需求

- **效能**:release profile `opt-level="z"` + `lto=true` + `codegen-units=1` + `strip=true` + `panic="abort"`;快取命中時 render < 5ms;發佈時附 `hyperfine` 實測數字(研究 §4.2-7)
- **安全**(全部對等 shell 版):快取目錄限 `$XDG_RUNTIME_DIR` → `$HOME/.cache` 鏈,逐層驗證 owner + 無 group/other 寫位 + 非 symlink,fail-closed(不安全→完全不讀寫快取);寫入一律 temp+rename 原子;所有外部字串(payload、快取檔、API 回應、tag_name)輸出前過濾控制字元;render 路徑絕不 panic(禁 `unwrap`/`expect`,clippy lint 強制)
- **依賴極簡**:`serde`/`serde_json`、`ureq`(rustls feature,blocking,無 tokio)、`unicode-width`;dev:`insta`、`tempfile`。每新增依賴需在 PR 說明理由

## 6. 架構

```
src/
├── main.rs          # 讀 stdin → Context 組裝 → render → print;頂層 catch 全部錯誤
├── input.rs         # serde structs + sanitization(§3.1)
├── config.rs        # env vars 解析與 clamp
├── cachedir.rs      # 目錄安全驗證、atomic write、mkdir lock(§5)
├── gitstatus.rs     # porcelain v2 解析 + 快取(trait GitRunner 注入)
├── oauth.rs         # token 來源鏈 + usage API + 快取(trait HttpClient / CredentialStore 注入)
├── ttl.rs           # cache TTL 狀態機
├── render/
│   ├── mod.rs       # 區塊組裝、分隔符、折行
│   ├── blocks.rs    # workspace / model / context / cache / limits 各區塊
│   ├── meter.rs     # bar/dots + 配色階梯
│   └── color.rs     # ANSI 常數
├── width.rs         # 顯示寬度計算 + ANSI strip
└── update.rs        # GitHub releases 檢查 + semver 比較
```

外部效應全走 trait(`GitRunner`、`HttpClient`、`CredentialStore`、`Clock`),測試注入 mock;`Clock` 注入使 TTL/閃爍/快取年齡可確定性測試。子行程呼叫(git/security/secret-tool)以 thread+mpsc 包 timeout。

## 7. 發佈與安裝

### 7.1 Release assets(每版 14 個檔案)

```
cc-statusline-darwin-arm64.tar.gz        (+ .sha256)
cc-statusline-darwin-x64.tar.gz          (+ .sha256)
cc-statusline-linux-arm64-musl.tar.gz    (+ .sha256)
cc-statusline-linux-x64-musl.tar.gz      (+ .sha256)
cc-statusline-win32-x64.zip              (+ .sha256)
cc-statusline-win32-arm64.zip            (+ .sha256)
cc-statusline-npm.tgz                    # 檔名不含版本 → releases/latest/download URL 恆定
install.sh                                # 與同版 release assets 一起提供
```

### 7.2 安裝路徑(README 主打順序)

1. `npm install -g https://github.com/gn00678465/StatusLine/releases/latest/download/cc-statusline-npm.tgz` — postinstall:`process.platform`/`arch` 偵測(win32 判 `.exe`)→ 下載對應 asset + `.sha256` → 驗證 → 解壓至 `~/.claude/cc-statusline/`;`--ignore-scripts` 環境給出明確錯誤訊息
2. `curl -fsSL .../releases/latest/download/install.sh | sh` — 同邏輯,免 Node/免 PAT(install.sh 也掛在 assets 上)
3. 手動下載解壓

settings.json(全平台同一份):

```json
{ "statusLine": { "type": "command", "command": "~/.claude/cc-statusline/cc-statusline", "refreshInterval": 1 } }
```

### 7.3 CI(GitHub Actions)

- `ci.yml`:push/PR — fmt + clippy(deny warnings)+ test(macOS/Linux/Windows 三 OS)+ `cargo-llvm-cov` 覆蓋率門檻 80%
- `release.yml`:tag `v*` — 6 target 交叉編譯(musl 用 cargo-zigbuild;交叉編譯 matrix 參考 napi-rs CI 樣板,D13 之下僅參考結構不複製)→ 打包 + sha256 → `npm pack` → 上傳全部 assets → 建立 GitHub Release

## 8. 測試策略(D12)

- **單元**:token 格式化、寬度計算、meter/配色階梯、TTL 狀態機、semver 比較、porcelain v2 解析、OAuth 回應解析(`weekly_scoped` 泛化含 0/1/多條目)、sanitization
- **整合**:`tests/fixtures/*.json`(5 份現成 + 補 `xhigh`/`effort 缺失`/1m context/多 weekly_scoped 案例)灌 stdin,mock trait 注入,`insta` snapshot 驗最終輸出;邊界:空 stdin、非 JSON、超寬折行、快取目錄不安全
- **對等驗證**:同 fixture 下 Rust 輸出 vs shell 版輸出 diff(僅 D9 三項修正與 Fable 泛化允許差異),已於 shell 版退役前完成最終驗收
- bash 測試 harness 於 ticket 13 移除 shell script 時一併退役;fixtures 與 Rust integration snapshots 保留

## 9. 驗收條件(v2.0.0 合併門檻)

1. [x] 全部 fixtures snapshot 通過;對等驗證 diff 僅含預期差異(已於 shell 退役前完成)
2. [x] 覆蓋率 ≥80%;clippy deny warnings;無 unwrap/expect 於 render 路徑
3. [x] 6 target 於 CI 全綠;release dry-run 產出 §7.1 全部 14 個 assets
4. [ ] macOS 實機 release 安裝與真實 Claude Code session(本地 npm/installer/mock checksum 驗證已通過;live release/session 由使用者處理)
5. [x] `hyperfine` 實測數字寫入 README(暖快取約 2 ms;冷快取約 12 ms)
6. [x] README/CHANGELOG 更新;shell script 及 bash 測試自 main 移除
   [ ] `v1-final` tag(由使用者處理)

> Ticket 13 刻意不處理 LICENSE 選擇、`v1-final` tag 與真實 Claude Code
> session；其餘條款依目前 branch 狀態標註如上。
