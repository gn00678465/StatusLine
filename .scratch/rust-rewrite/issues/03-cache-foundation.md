# 03 — 快取基礎層

Status: done
Type: task
Blocked by: 01

## 範圍

`cachedir.rs`(spec §5 安全需求,對等 shell 版 `_dir_is_safe`/`_atomic_write`):

- 目錄鏈驗證:`$XDG_RUNTIME_DIR` → `$HOME/.cache` → `<base>/StatusLine`;每層:實目錄、非 symlink、owner 是自己、無 group/other 寫位(Unix mode bits;Windows 上以使用者 profile 目錄視為安全)
- fail-closed:驗證不過 → 回 `CacheDir::Unsafe`,所有讀寫變 no-op
- atomic write:同目錄 tempfile(0600)+ rename
- mkdir 原子鎖 + stale 清理(帶 max age 參數)
- `Clock` trait(`now_epoch()`)供全 crate 注入

## 驗收

- [x] 單元測試(tempfile):正常鏈、寬鬆權限拒絕、symlink 拒絕、鎖競爭、stale 鎖清理
- [x] Unsafe 狀態下所有操作 no-op 且不 panic

## Comments

- 完成 `CacheDir`：優先安全的 `XDG_RUNTIME_DIR`，否則驗證 HOME → `.cache` → `StatusLine` 的每一層；任何非實目錄、symlink、非本人 owner 或 group/other writable 皆 fail-closed 成 `Unsafe`。Windows 僅信任使用者 profile 路徑中的實目錄。
- 完成檔名限制的 safe read、同目錄私有 temporary file（Unix 0600）+ rename atomic write、mkdir atomic lock、依 `Clock::now_epoch()` 判斷的 stale lock 清理，以及釋放時移除 lock 的 `CacheLock`。
- TDD 證據：安全鏈、鎖競爭、atomic write、unsafe no-op 各先 RED（缺少 `CacheDir`、`Clock`、`atomic_write`、`read`），最小實作後 GREEN；另補 HOME/.cache/StatusLine 三層寬鬆權限與 symlink 回歸測試。
- 本機驗證成功：`cargo fmt --check`、`cargo test --all-targets`（17 unit + 2 integration 全過）、`cargo clippy --all-targets -- -D warnings`、`cargo build --release`。
- 依賴：runtime `libc` 僅用於 Unix `geteuid()`，以實作 spec 要求的目前使用者 owner 驗證；`tempfile` 為 dev-dependency，僅用於隔離檔案系統測試。
- 偏離：無。
- 驗證(orchestrator, 2026-08-23):`is_safe_directory` 與 shell `_dir_is_safe` 逐項對等(euid、`mode & 0o022`、symlink、fail-closed、逐層含 weak leaf);entry name 單一 component 防 traversal、read 拒 symlink、`sync_all` 落盤、Windows rename 可覆蓋。fmt/clippy/test 獨立重跑全綠(17+2)。`libc`(geteuid)依賴核准。→ done
- 後續硬化(隨 ticket 04 一併做):目錄建立改用 `DirBuilderExt::mode(0o700)` 於建立當下賦權,消除 create→chmod 之間的短暫預設權限窗口(對等 shell 版 umask 077 的「自始私有」語意);`ensure_safe_child_directory` 與 `try_lock` 兩處。
- 流程註記(第二次):08ff861 仍掃入非本 ticket 的 docs/agents 修改。
