# Git staged/unstaged 狀態與高頻 refresh 效能研究

日期：2026-08-09

## 結論摘要

目前 `StatusLine` 的 Git 區塊每次 refresh 啟動 2 個 Git process：先取得 branch，再以 `git diff --numstat` 計算行數。後者只比較 working tree 與 index，因此 **完全看不到 staged change**；untracked、binary、conflict 與 ahead/behind 也不在顯示範圍內。`head -n 200` 會讓結果低估，而且只能截短輸出，不能保證 Git 在產生前 200 筆以前沒有先完成昂貴的 diff/rename 計算。

參考專案中真正值得採用的效能措施是 `best-claude-hud` 與 `claude-statusline` 使用的 `--no-optional-locks`。但不應照搬 `best-claude-hud` 的 process fan-out：它在一般 branch 每次 refresh 啟動 5 個 Git process，顯示 SHA 時為 6 個，detached HEAD 時還會再多 1 個。

建議 `StatusLine` 改為：

1. 以 **單次** `git status --porcelain=v2 --branch` 同時取得 branch 與 staged/unstaged 狀態。
2. 預設使用 `--no-optional-locks --untracked-files=no --ignore-submodules=dirty --no-renames`，避免背景 refresh 寫 index、掃描 untracked tree、遞迴檢查 submodule dirty state，以及 rename similarity 計算。
3. 顯示 staged/unstaged 的「檔案數」，不要在每次 refresh 計算 diff「行數」。兩種數字語意不同，UI 應明確標示，例如 `S2 W1`，不能沿用看似 added/deleted lines 的 `[+2|-1]`。
4. 再加一層預設約 2 秒、可設定/停用的短 TTL cache，吸收同一時間內的連續 redraw；Git timeout 時沿用 last-known-good，不讓 Git 卡住造成 statusline 消失或閃爍。

## 研究範圍與版本快照

本研究以本機 clone 的原始碼為主，固定在以下 commit：

| 專案 | Commit | 實作 |
| --- | --- | --- |
| `GaoSSR/best-claude-hud` | [`4e6228b`](https://github.com/GaoSSR/best-claude-hud/tree/4e6228bfeea1215e66da85def500e348808458b6) | Rust |
| `benabraham/claude-code-status-line` | [`a2cfda1`](https://github.com/benabraham/claude-code-status-line/tree/a2cfda1b135d925b064922c6524c5e779ef9252d) | Python |
| `nilbuild/claude-statusline` | [`ea02c0e`](https://github.com/nilbuild/claude-statusline/tree/ea02c0e6dcd532fea6056f7eec2b7545b3666248) | Bash |
| `gn00678465/StatusLine` | [`64948ed`](https://github.com/gn00678465/StatusLine/tree/64948ed99ca4cf7587640e51fc2c1ed4f03a5377) | Bash |

## 每次 refresh 的 Git subprocess

以下數字以預設會顯示 Git 區塊、目前目錄位於一般 Git branch 為前提。

| 專案 | 一般情況 | Git commands | timeout | cache | `--no-optional-locks` |
| --- | ---: | --- | --- | --- | --- |
| `best-claude-hud` | 5 | `rev-parse --git-dir`; `branch --show-current`; `status --porcelain`; 兩次 `rev-list --count` | 無 | 無 | 每個 command 都有 |
| `claude-code-status-line` | 4 | `branch --show-current`; `status --porcelain=v1`; `stash list`; `rev-list --left-right --count` | 每個 0.3 秒 | Git 無 cache | 無 |
| `claude-statusline` | 3 | `rev-parse --is-inside-work-tree`; `symbolic-ref --short HEAD`; `status --porcelain` | 無 | 無 | 只有 `status` 有 |
| 目前 `StatusLine` | 2 | `rev-parse --abbrev-ref HEAD`; `diff --numstat` | 只有 diff 嘗試 3 秒 | Git 無 cache | 無 |

補充：

- `best-claude-hud` 的 `branch --show-current` 沒結果時會再執行 `symbolic-ref`，所以 detached/unusual HEAD 為 6 個 process；`show_sha` 再增加一次 `rev-parse --short=7 HEAD`。來源見 [`git.rs` L42-L164](https://github.com/GaoSSR/best-claude-hud/blob/4e6228bfeea1215e66da85def500e348808458b6/src/core/segments/git.rs#L42-L164)。
- `claude-code-status-line` 的 `git_branch` 與 `git_status` 都在預設 segment 清單。即使 main/master 最後因 `hide_default=1` 不顯示，仍會先執行 branch command。Git 實作見 [`claude-code-status-line.py` L512-L605](https://github.com/benabraham/claude-code-status-line/blob/a2cfda1b135d925b064922c6524c5e779ef9252d/claude-code-status-line.py#L512-L605)，預設 segment 見 [L73](https://github.com/benabraham/claude-code-status-line/blob/a2cfda1b135d925b064922c6524c5e779ef9252d/claude-code-status-line.py#L73)。
- `claude-statusline` 把所有 staged、unstaged 與 untracked 狀態折疊成一個 `*`。來源見 [`bin/statusline.sh` L139-L146](https://github.com/nilbuild/claude-statusline/blob/ea02c0e6dcd532fea6056f7eec2b7545b3666248/bin/statusline.sh#L139-L146)。
- 目前 `StatusLine` 的 branch command 沒有 timeout；diff 使用 `_run_with_timeout 3`，但 macOS 沒裝 GNU `timeout`/`gtimeout` 時會直接無 timeout 執行。來源見 [`claudeStatusLine.sh` L129-L147](https://github.com/gn00678465/StatusLine/blob/64948ed99ca4cf7587640e51fc2c1ed4f03a5377/claudeStatusLine.sh#L129-L147) 與 [L244-L265](https://github.com/gn00678465/StatusLine/blob/64948ed99ca4cf7587640e51fc2c1ed4f03a5377/claudeStatusLine.sh#L244-L265)。

非 Git 目錄時，`best-claude-hud` 與 `claude-statusline` 會在第一個 repository check 停止；目前 `StatusLine` 也會在 branch command 沒輸出後跳過 diff。Python 參考案的 branch 與 status 是兩個獨立 renderer，因此預設仍會各試一次，`git status` 失敗後才不執行 stash/ahead-behind。

## 狀態語意比較

| 專案 | branch | staged | unstaged | untracked | conflict | stash | ahead/behind |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `best-claude-hud` | 名稱；detached 顯示固定文字 | 有偵測，折疊成 dirty | 有偵測，折疊成 dirty | 有，預設完整掃描 | `UU`/`AA`/`DD` 顯示 warning | 無 | 有，兩次 `rev-list` |
| `claude-code-status-line` | 名稱 | 解析 XY 的 X，最後只顯示 `+` | 解析 XY 的 Y，最後只顯示 `!` 等符號 | 有，預設完整掃描 | 有 | 有 | 有，一次 `rev-list` |
| `claude-statusline` | 名稱 | 有，但只折疊成 `*` | 有，但只折疊成 `*` | 有，但只折疊成 `*` | 只折疊成 `*` | 無 | 無 |
| 目前 `StatusLine` | 名稱 | **無** | tracked text 的 added/deleted lines | **無** | **無可靠表示** | 無 | 無 |

Git 官方說明中，不帶 commit 的 `git diff` 比較 working tree 與 index，也就是只看「尚未 staged」的變更；`git diff --cached` 才比較 index 與 HEAD；`git diff HEAD` 則比較最終 working tree 與 HEAD。因此目前的 `git diff --numstat` 不可能看見 staged-only change。[Git diff 官方文件](https://git-scm.com/docs/git-diff.html#_description)

`--numstat` 的數字是 added/deleted **lines**，不是 changed files；binary 檔案以 `-` 表示，現有的純數字 regex 會直接忽略。[Git `--numstat` 格式](https://git-scm.com/docs/git-diff.html#Documentation/git-diff.txt---numstat)

## 為何高頻 `git status` 會干擾其他 Git 操作

Git 官方特別指出：`git status` 預設會 refresh index，並把更新後的 stat cache 寫回 index。這個寫入只是最佳化，不是查詢正確性所必需；背景程式持有的 index lock 卻可能與同時執行的其他 Git process 衝突。因此官方直接建議背景 status 使用 `git --no-optional-locks status`。[Git status：Background refresh](https://git-scm.com/docs/git-status#_background_refresh)

`--no-optional-locks` 等同 `GIT_OPTIONAL_LOCKS=0`，會略過需要 lock 的非必要副作用，例如 status refresh index。它不代表「完全不用讀 index」，也不會免除 tracked-file 掃描，所以仍需減少 command 次數與 refresh 頻率。[Git 全域選項與環境變數](https://git-scm.com/docs/git#Documentation/git.txt---no-optional-locks)

另一個常見瓶頸是 untracked enumeration。Git 官方稱 `--untracked-files=no` 是最快選項；若產品一定要顯示 untracked，則可讓使用者自行啟用 `core.untrackedCache`，搭配 `core.fsmonitor` 可進一步避免重掃沒有變動的目錄。[Git status：Untracked files and performance](https://git-scm.com/docs/git-status#_untracked_files_and_performance)、[Git update-index：Untracked cache / FSMonitor](https://git-scm.com/docs/git-update-index#_untracked_cache)

## 方案比較

### A. 單次 porcelain v2，計 staged/unstaged 檔案數（建議）

建議 command：

```bash
git --no-optional-locks -C "$cwd" status \
  --porcelain=v2 \
  --branch \
  --no-ahead-behind \
  --untracked-files=no \
  --ignore-submodules=dirty \
  --no-renames
```

一次輸出即可提供：

- `# branch.head <branch>`：branch 名稱；detached 時為 `(detached)`。
- `# branch.oid <oid>`：需要時可作為 detached HEAD 的短 SHA 來源，不必再啟動 `rev-parse`。
- `1 <XY> ...`、`2 <XY> ...`：tracked ordinary/rename records；X 是 index（staged），Y 是 working tree（unstaged），未變更以 `.` 表示。
- `u <XY> ...`：unmerged/conflict record。
- 若未來要 ahead/behind，移除 `--no-ahead-behind` 後可從 `# branch.ab +N -N` 解析，不需要另外執行 `rev-list`。
- 若未來要 stash，加入 `--show-stash` 後可從 `# stash N` 解析，不需要另外執行 `stash list`。

Porcelain v2 的 branch header、XY 與 conflict record 都是官方定義的 machine-readable format。[Git status：Porcelain v2](https://git-scm.com/docs/git-status#_porcelain_format_version_2)

建議計數規則：

- `1`/`2` record 的 X 不是 `.`：staged files 加 1。
- `1`/`2` record 的 Y 不是 `.`：unstaged files 加 1。
- 同一檔案為 `MM` 時兩邊都加 1，因為它確實同時有 staged 與 staged 後的新修改。
- `u` record：conflict files 加 1，不硬塞入 staged/unstaged。
- 預設 `--untracked-files=no`，因此不承諾 untracked count；若讓使用者 opt-in，再解析 `? <path>`。

優點是 process 從 2 降為 1、修正 staged invisibility，而且不需要內容級 diff。代價是顯示改為 changed-file count，不再是 added/deleted-line count；這是刻意調整語意，不是等價替換。

### B. `git diff HEAD --numstat`，保留整體行數

`git diff HEAD --numstat` 可以在一個 diff process 中比較 HEAD 與最後的 working tree，因而涵蓋 staged + unstaged 的**最終淨差異**。但它有以下限制：

- 無法分開 staged 與 unstaged。
- 同一檔案 staged 後又修改時，數字是 HEAD 到 working tree 的淨結果，不等於兩個階段各自 numstat 相加。
- 看不到 untracked。
- binary 仍沒有行數。
- unborn branch 沒有 HEAD，必須另做 fallback。
- branch 仍需另一個 process，總數沒有降到 1。

若需求只是「這個 working tree 相對 HEAD 最後會改幾行」，此方案語意成立；若需求是使用者特別提出的 Git stage 狀態，則不適合。

### C. 分別執行兩次 numstat，精確拆 staged/unstaged 行數

需要：

```bash
git diff --cached --numstat --no-renames
git diff --numstat --no-renames
```

再加 branch 至少 3 個 process。它能保留行數並拆 stage，但每次 redraw 需做兩次內容 diff，正是本研究要避免的成本。除非把這組慢資料放到較長 TTL（例如 5–10 秒）的獨立 cache，否則不建議。

### D. 只加 cache，不改 command

短 TTL 能把 burst 中的多次呼叫合併，但保留目前 staged 看不到、binary/untracked 不完整與前 200 筆低估等問題。它只能是輔助層，不能代替 command/語意修正。

## 短 TTL cache 設計

Statusline script 每次都是新的 Bash process，無法使用 process-local memory cache；若要跨 refresh，必須使用檔案 cache 或常駐 daemon。對目前單檔 Bash 專案，檔案 cache 較符合架構。

建議：

- 預設 TTL 2 秒，提供環境變數覆寫，`0` 代表停用。
- cache value 包含 sanitized cwd、timestamp、branch、staged、unstaged、conflict；讀取時 cwd 不同必須 miss，避免多 repo 間顯示錯誤資料。
- 使用現有已驗證 owner/mode 的 `StatusLine` cache directory、atomic write 與 mkdir lock，不另建不安全的 `/tmp` predictable path。現有安全 cache 初始化目前位於 Git render 之後，實作時需把初始化提前或延後 workspace render。
- refresh lock 已被其他 process 取得時使用同 cwd 的 stale value；沒有同 cwd stale value 才執行自己的查詢，不能把另一個 repo 的值拿來顯示。
- Git 成功才覆寫 cache。timeout、non-repo、parse failure 要區分：non-repo 可以短暫 negative-cache；timeout/parse failure 保留 last-known-good。
- TTL 只承諾 eventual freshness：staged/branch/working tree 改變後最多可能延遲約 2 秒。若這個延遲不可接受，就停用外層 cache，但仍保留單次 porcelain 與 `--no-optional-locks`。

不建議用 `.git/index` mtime 作唯一 invalidation：它可捕捉 staged 變動，卻看不到一般 working-tree file edit；worktree 的 `.git` 還可能是指向 common dir 的文字檔。為了找出正確 git dir 再多啟動一次 `rev-parse`，也會抵銷單次查詢的目標。

## 建議實作順序

1. 新增 staged-only、unstaged-only、同檔 `MM`、conflict、untracked-only、detached HEAD、unborn branch、non-repo 的 fixture/integration tests。
2. 用單次 porcelain v2 取代 branch + numstat，顯示 staged/unstaged file counts；更新 README 的數字語意。
3. 加上 `--no-optional-locks`、`--untracked-files=no`、`--ignore-submodules=dirty`、`--no-renames`。
4. 保留 timeout 防線，但不要把目前 macOS 無 GNU timeout 的 fallback 描述成 hard deadline。
5. 加 2 秒 TTL cache、atomic write、lock 與 last-known-good；測試同 repo burst、兩個 repo concurrent refresh、cache corruption、timeout fallback。
6. 用大型 repo 實測 p50/p95 wall time、Git process 數與 `index.lock` 衝突，再決定是否提供 opt-in untracked/ahead-behind。

## 修改前後的 burst benchmark

以固定在修改前 `HEAD` 的同一個乾淨 worktree 作為 Git 查詢目標，修改前後各跑 3 輪、每輪連續執行完整 statusline 25 次。兩者使用相同 stdin 與 mock API、各自獨立的安全 cache directory，執行順序交錯；以 `GIT_TRACE2_EVENT` 計算 Git process start，表中時間取 3 輪中位數：

| 實作 | 3 輪 wall time | 25 次 redraw 中位數 | Git process starts / 輪 |
| --- | --- | ---: | ---: |
| 修改前：branch + `diff --numstat`，無 Git cache | 5.264 / 5.180 / 5.421 s | 5.264 s | 50 |
| 修改後：單次 porcelain v2，預設 2 秒 cache | 2.769 / 2.751 / 2.769 s | 2.769 s | 2 |

在此測試中，完整 statusline burst 的中位時間減少 **47%**，Git process start 從 50 降為 2（減少 **96%**）。這是本機 `/d` filesystem 上的小型 repo benchmark，用來驗證實作確實降低 Git 啟動與整體延遲；實際數字仍會隨 repo 大小、filesystem、OS 與 refresh 間隔而變化。

## 最終判斷

`best-claude-hud` 證明了背景 status 應全面使用 `--no-optional-locks`，但它不是 subprocess 數量的好範本；Python 參考案的 0.3 秒 timeout 是良好的 failure containment，卻仍付出 4 個 process 與 untracked scan；Bash 參考案較簡單，但仍有 3 個 process。

對 `StatusLine` 最合適的組合是：**單次 porcelain v2 + staged/unstaged 檔案數 + no optional locks + 預設不掃 untracked + 2 秒 TTL**。這比保留每次 `numstat` 行數更直接解決使用者提出的 stage 正確性與高頻 Git 查詢干擾問題。
