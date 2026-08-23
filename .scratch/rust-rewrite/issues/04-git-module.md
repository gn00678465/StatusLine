# 04 — Git 狀態模組

Status: review
Type: task
Blocked by: 03

## 範圍

`gitstatus.rs`(spec §4.1-1,語意對等 shell 版 `_collect_git_status`):

- `GitRunner` trait 注入;真實實作:單次 `git --no-optional-locks -C <cwd> status --porcelain=v2 --branch --no-ahead-behind --untracked-files=no --ignore-submodules=dirty --no-renames`,thread+mpsc 包 1 秒 timeout(spec §4.2-3)
- porcelain v2 解析:`# branch.head`(`(detached)` → `detached`)、`1 `/`2 ` 行的 XY 計 staged/unstaged、`u ` 行計 conflicted
- per-session 快取:TTL 依 config、repo 路徑一致性檢查、timeout 時回 stale、mkdir 刷新鎖(5 秒 stale)

## 驗收

- [x] 單元測試:porcelain 解析(clean/staged/mixed/conflict/detached/空 repo)
- [x] 快取測試(mock Clock + GitRunner):命中、過期刷新、timeout fallback stale、TTL=0 停用

## Comments

- 完成 deep `GitStatusCollector` module：以單一 `collect(repo, session, ttl)` interface 封裝 `GitRunner`、1 秒 thread+mpsc timeout、安全快取、5 秒 stale refresh lock 與 stale fallback。`CommandGitRunner` 執行一次指定的 porcelain-v2 Git command，並捕獲 stdout/stderr，避免污染 statusline stderr。
- 完成 porcelain v2 解析：`# branch.head`、`(detached)`、`1 `/`2 ` XY` staged/unstaged 計數、`u ` conflict 計數；分支字串會過濾控制、BiDi 與 zero-width 字元。空 repo 的 `(initial)` 依 shell 對等語意保留。
- 快取驗證包括同 repo/session hit、過期刷新、repo 路徑不符刷新、TTL=0 完全停用，以及實際 mpsc timeout 回 stale（不重新寫入 stale cache），對應 shell rc=124 fallback 語意。
- TDD 證據：mixed porcelain 測試先因缺少 `parse_porcelain` RED；cache-hit 測試先因缺少 collector/runner seam RED；其後再以 mock `Clock`/`GitRunner` 補齊過期、timeout、TTL 與 repo mismatch 回歸測試。
- 本機驗證成功：`cargo fmt --check`、`cargo test --all-targets`（24 unit + 2 integration 全過）、`cargo clippy --all-targets -- -D warnings`、`cargo build --release`。
- 偏離：無。
