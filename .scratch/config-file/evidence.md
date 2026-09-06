# Evidence Report — 使用者設定檔 `~/.config/cc-statusline/config.toml` (Tier 2)

- `headline`: GATE PASSED — suite health SUBSTITUTED（3 次重跑，無隨機順序）；changed-line coverage 與 mutation 對該層 DEPENDENCY UNMET；supply-chain 弱點稽核 UNAVAILABLE；intent confirmed；git_facts complete；reproducible
- `command`: `evidence`
- `contract`: applied（evidence-first；`AGENTS.md`「Autonomy and approval」「Evidence and completion」段落覆寫 shared workflow 的衝突條款，本報告依其執行）
- `scope`: config-file
- `change_set`: `main...HEAD`（`b805014...5ef0258`）
- `base`: `main` = `b805014394930d6c51b9701fa7bc8e0ed774bc53`
- `report_language`: zh-TW
- `intent_status`: confirmed
- `intent_source`: `specs/config-file/SPEC.md`，`spec_version: v3`，`status: approved`；核准語句逐字記錄於該檔 Approval 區塊（「核准 spec v2」2026-09-06、「核准 spec v3」2026-09-06）
- `ordering`: tests-first — 5 個行為批次各有 `test: RED` commit 先於其 `feat: GREEN` commit（3229150→e4086a8、258f10b→9232191、069f633→dfb4b99、b1ccbc7→2a7fe68、1847aaa→6eea4d7）；另有 6 個一寫即過的測試 commit（764591e、0eb2ff0、2a89621、29dbaef、89716ed、2577971）為 regression armor，各自 commit message 記錄一次性 mutant 驗證
- `git_facts`: complete（base 可解析、非 shallow、baseline 與 RED 皆自 git 重建）
- `source_state`: `5ef0258689d3d8a0af414d5458553e01ba8f9166`（`tools/gate/source_state.sh`；final run 前後皆為此值，逐字比對相同）
- `source_state_exclusions`: none — verifier 已提交（`tools/gate.sh` 與 `tools/gate/*` 皆為 tracked）
- `toolchain`: `tools/gate/versions.txt` 釘住並於 versions 層逐一比對：cargo 1.97.1 (c980f4866 2026-06-30)、rustc 1.97.1 (8bab26f4f 2026-07-14)、cargo-llvm-cov 0.9.0、cargo-mutants 27.1.0、Python 3.13.15；Rust 依賴由 `Cargo.lock` 釘住
- `entry_point`: `sh tools/gate.sh --base main --scope config-file`，於 `git worktree add --detach ../status-line-gate 5ef0258` 的乾淨工作樹執行（Windows 11，Git Bash）
- `reproducibility`: reproducible
- `changed_unit_command`: `git diff -U0 $(git merge-base main HEAD) HEAD -- src tests | grep -E '^[-+].*\b(fn|struct|enum|impl|mod|const|static)\b'`
- `changed_unit_granularity`: symbol

## Baseline

`tools/gate/replay_history.sh` 在 `b805014` 的 detached worktree 執行 `cargo test --all-targets --no-fail-fast`：

- status: ok — `test result: ok. 70 passed; 0 failed`（unit）、`test result: ok. 2 passed; 0 failed`（integration）
- failing tests: none — base was green

## Changed unit → Test

Rows 由 diff 推導（symbol 粒度）。`n-a` 列為文件、測試 helper 與純可見性變更。

| Changed unit | Test | Status |
|---|---|---|
| src/cachedir.rs::home_directory（private → pub(crate)，本體不變） | tests/cc_statusline.rs::renders_with_defaults_when_no_home_directory_exists（經 Config::load）；既有 cachedir 測試 | pass |
| src/config.rs::MAX_CONFIG_FILE_BYTES、PARSER_STACK_SIZE_BYTES（新常數） | src/config.rs::oversized_config_file_yields_defaults_with_diagnostic（16384 通過／16385 拒絕／16387 多位元組邊界）；tests/cc_statusline.rs::deeply_nested_config_file_does_not_crash | pass |
| src/config.rs::FileConfig（新 struct，serde default，無 deny_unknown_fields） | src/config.rs::unknown_file_keys_are_ignored、malformed_config_file_yields_defaults_with_diagnostic | pass |
| src/config.rs::EnvValues（新 struct） | src/config.rs::env_usage_style_overrides_file、empty_env_value_is_absent_so_file_applies、env_only_behaviour_is_unchanged_without_file | pass |
| src/config.rs::config_file_path | src/config.rs::config_path_prefers_xdg_config_home_then_home_dot_config_then_none；mutation kill sample 1（3/3 caught） | pass |
| src/config.rs::read_config_file — NotFound 分支 | src/config.rs::missing_config_file_yields_defaults_without_diagnostic | pass |
| src/config.rs::read_config_file — open 其他錯誤分支 | src/config.rs::unreadable_config_path_yields_defaults_with_diagnostic（Windows：目錄在 open 失敗） | pass |
| src/config.rs::read_config_file — read_to_end 錯誤分支（config.rs:71） | 無（allowlist：Unix 上目錄在 read 才失敗，S10 於 Unix 覆蓋此行；本 gate 在 Windows 執行） | unverified |
| src/config.rs::read_config_file — 大小上限分支 | src/config.rs::oversized_config_file_yields_defaults_with_diagnostic | pass |
| src/config.rs::read_config_file — from_utf8 錯誤分支 | src/config.rs::non_utf8_config_file_yields_defaults_with_diagnostic | pass |
| src/config.rs::read_config_file — 解析成功／解析錯誤分支 | src/config.rs::malformed_config_file_yields_defaults_with_diagnostic、file_usage_style_dots_applies_when_env_absent；kill sample 2（11/11 caught） | pass |
| src/config.rs::parse_on_dedicated_thread — Ok(Ok)／Ok(Err) 分支 | 同上（所有經檔案解析的測試）；tests/cc_statusline.rs::deeply_nested_config_file_does_not_crash（4000 層巢狀，主執行緒 1 MiB 會溢位） | pass |
| src/config.rs::parse_on_dedicated_thread — join Err（config.rs:101）、spawn Err（config.rs:103） | 無（allowlist：防禦分支，需 mock 受測單元才能觸發） | unverified |
| src/config.rs::diagnostic | src/config.rs::malformed_config_file_yields_defaults_with_diagnostic（斷言含路徑）；tests/cc_statusline.rs::renders_status_line_and_warns_on_malformed_config_file（stderr 含 config.toml） | pass |
| src/config.rs::Config::resolve（取代 from_env） | src/config.rs::file_usage_style_dots_applies_when_env_absent、env_usage_style_overrides_file、empty_env_value_is_absent_so_file_applies、file_git_cache_ttl_applies_and_clamps_to_60、invalid_file_usage_style_falls_back_to_bar、file_cannot_set_columns、env_only_behaviour_is_unchanged_without_file | pass |
| src/config.rs::Config::load — Some 分支 | tests/cc_statusline.rs::renders_dots_meter_from_config_file、env_overrides_config_file_in_real_binary | pass |
| src/config.rs::Config::load — None 分支 | tests/cc_statusline.rs::renders_with_defaults_when_no_home_directory_exists | pass |
| src/config.rs::non_empty | src/config.rs::empty_env_value_is_absent_so_file_applies | pass |
| deleted: src/config.rs::Config::from_env | 唯一呼叫端 src/main.rs::StatusLineApp::system 改呼叫 Config::load；types 層 `cargo check --all-targets` 0 errors 證明無其他依賴 | pass |
| src/main.rs::StatusLineApp::system（改用 Config::load；diagnostic 以 `writeln!(stderr)` 輸出並忽略寫入錯誤） | tests/cc_statusline.rs::renders_status_line_and_warns_on_malformed_config_file（stderr）、renders_dots_meter_from_config_file（stderr 為空） | pass |
| Cargo.toml／Cargo.lock：新增 basic-toml 0.1.10 | supply-chain 層：lock 差異恰為 `basic-toml`；license `MIT OR Apache-2.0` | pass |
| README.md、CHANGELOG.md（文件） | — | n-a |
| tests/cc_statusline.rs helpers（isolated_command、write_config_file、run_with_fixture） | 由 7 個整合測試執行；不在 cargo-llvm-cov 報表範圍 | n-a |

## Stated claim → Test

Rows 來自 `specs/config-file/SPEC.md` v3（Scenarios 1–20 與 Must NOT）。

| Claim | Test | Status |
|---|---|---|
| S1 檔案 dots、env 未設 → Dots | src/config.rs::file_usage_style_dots_applies_when_env_absent | pass |
| S2 env 覆寫檔案（雙向） | src/config.rs::env_usage_style_overrides_file | pass |
| S3 空字串 env 視為未設定 | src/config.rs::empty_env_value_is_absent_so_file_applies | pass |
| S4 檔案 ttl 生效並夾到 60 | src/config.rs::file_git_cache_ttl_applies_and_clamps_to_60 | pass |
| S5 檔案無效 usage_style → bar、無 diagnostic | src/config.rs::invalid_file_usage_style_falls_back_to_bar | pass |
| S6 未知鍵忽略（真實 TOML） | src/config.rs::unknown_file_keys_are_ignored | pass |
| S7 檔案不能設 COLUMNS（真實 TOML） | src/config.rs::file_cannot_set_columns | pass |
| S8 檔案不存在 → 預設、無 diagnostic | src/config.rs::missing_config_file_yields_defaults_without_diagnostic | pass |
| S9 語法／型別錯誤四種 → 預設、diagnostic 含路徑 | src/config.rs::malformed_config_file_yields_defaults_with_diagnostic | pass |
| S10 路徑是目錄 → 預設、diagnostic | src/config.rs::unreadable_config_path_yields_defaults_with_diagnostic | pass |
| S11 路徑決議 XDG → ~/.config → None | src/config.rs::config_path_prefers_xdg_config_home_then_home_dot_config_then_none | pass |
| S12 無檔案時 env 行為不變 | src/config.rs::env_only_behaviour_is_unchanged_without_file；既有 parses_usage_style_ttl_and_columns_with_clamps_and_fallbacks 逐字未改（unchanged_tests.py） | pass |
| S13 真實 binary：檔案 dots → ●、stderr 空 | tests/cc_statusline.rs::renders_dots_meter_from_config_file；real-execution 層 `dots-from-file` | pass |
| S14 真實 binary：格式錯誤 → 正常輸出、stderr 含 config.toml | tests/cc_statusline.rs::renders_status_line_and_warns_on_malformed_config_file；real-execution 層 `malformed-file` | pass |
| S15 真實 binary：env 覆寫檔案 | tests/cc_statusline.rs::env_overrides_config_file_in_real_binary | pass |
| S16 16385 bytes 拒絕、16384 bytes 通過 | src/config.rs::oversized_config_file_yields_defaults_with_diagnostic | pass |
| S17 真實 binary：4000 層巢狀不中止 | tests/cc_statusline.rs::deeply_nested_config_file_does_not_crash | pass |
| S18 真實 binary：無家目錄 → 預設 | tests/cc_statusline.rs::renders_with_defaults_when_no_home_directory_exists | pass |
| S19 非 UTF-8 → 預設、diagnostic | src/config.rs::non_utf8_config_file_yields_defaults_with_diagnostic | pass |
| S20 大小檢查先於 UTF-8 解碼（16387 bytes 多位元組邊界 → diagnostic 含 16384） | src/config.rs::oversized_config_file_yields_defaults_with_diagnostic | pass |
| Must NOT：無檔案且 env 不變時輸出不變 | tests 層（既有 72 測試全數通過）；unchanged_tests.py 逐字比對 src/main.rs 測試模組（含 inline snapshot）與三個既有測試函式 | pass |
| Must NOT：stdout 只有 statusline | must-not-scans（產品新增行無 print!/println!）；S13/S17/S18 stderr 為空；S14 diagnostic 只在 stderr | pass |
| Must NOT：任何設定檔內容不得 panic／非 0／stdout 空（語法、型別、超大、非 UTF-8、目錄、深巢狀） | S9、S10、S14、S16、S17、S19、S20；must-not-scans（新增行無 unwrap/expect/panic!/todo!/unimplemented!）；clippy `-D warnings` 含 `expect_used`/`unwrap_used` | pass |
| Must NOT：不從檔案讀 COLUMNS | S7 | pass |
| Must NOT：不建立／寫入 ~/.config | must-not-scans（產品新增行無 fs::write/create_dir*/remove*/rename/File::create/OpenOptions） | pass |
| Must NOT：不動 CLAUDE_CONFIG_DIR／OAuth／cache 語意 | diff 未觸及 src/oauth.rs、src/update.rs；src/cachedir.rs 只改一個 fn 的可見性；既有 oauth/cachedir/update 測試通過 | pass |
| Must NOT：只新增 basic-toml | supply-chain 層 lock 差異；unchanged_tests.py `ALLOWED_NEW_CRATES` | pass |
| Must NOT：熱路徑無新子程序／網路／多於一次讀檔 | must-not-scans（產品新增行無 Command::new／ureq::）；capability diff 只新增 std::fs::File、std::io::{Read, Write} 等 std 項目；read_config_file 單次 open+read | pass |
| Must NOT：不改既有 fixture／測試斷言 | unchanged_tests.py（`git diff -- tests/fixtures` 為空；三個既有測試函式逐字相同） | pass |
| Must NOT：不動版號 | unchanged_tests.py（Cargo.toml `[package] version` 相同） | pass |

## RED reconstruction

`tools/gate/replay_history.sh main config-file`（腳本版本 `2eee553`，對 `b805014..2eee553` 共 33 個 commit 執行）：每個 commit 在 detached worktree 執行全套件（`--no-fail-fast`）；`test: RED` 開頭者必須失敗，其餘必須通過。結果 `replay ok`，0 mismatch；完整表格為執行時產出的 `.gate/config-file/red.md`（未入版控），摘錄如下。

| Test（RED commit） | Result at base / at RED commit | Note |
|---|---|---|
| 3229150 test: RED — S1–S7、S12（8 個 config::tests） | failed（assertion） | 全部 8 個以斷言失敗；GREEN e4086a8 全通過 |
| 258f10b test: RED — S8–S11（4 個 config::tests） | failed（assertion，stub `todo!`） | GREEN 9232191 全通過 |
| 069f633 test: RED — S13–S15（tests/cc_statusline.rs） | failed（S13、S14 assertion） | S15 一寫即過：實作者以一次性 mutant（meter_style 強制 Dots）觀察失敗後還原，記於 commit message；作為 regression armor |
| b1ccbc7 test: RED — S16、S17 | failed（重放記錄 `oversized_config_file_yields_defaults_with_diagnostic` 與 `deeply_nested_config_file_does_not_crash` 皆失敗；後者為子程序 stack overflow 中止，`status.success()` 斷言失敗） | GREEN 2a7fe68 全通過 |
| 1847aaa test: RED — S20/G1 | failed（assertion：diagnostic 為 UTF-8 錯誤而非上限） | GREEN 6eea4d7 全通過 |
| 764591e、0eb2ff0（S6、S7 改為真實 TOML）、2a89621（PATH 隔離）、29dbaef（移除不可達 panic 分支）、89716ed（S18）、2577971（S19） | passed | pre-existing behaviour, kept as regression armor；每個 commit message 記錄一次性 mutant（deny_unknown_fields／FileConfig.columns／load None→panic／from_utf8 分支）觀察失敗後還原 |

## Gate (final fresh run)

全部數字來自 `sh tools/gate.sh --base main --scope config-file` 於 `5ef0258` 乾淨 worktree 的單一次執行（`.gate/config-file/` 各層 log）。

| Layer | Command | Threshold (what makes this pass) | Result |
|---|---|---|---|
| Versions | `check_versions tools/gate/versions.txt` | 5 個工具 `--version` 首行與釘住值逐字相同 | 5/5 相同 |
| Self-test | `sh tools/gate/selftest.sh` | 每個 home-grown check 的負向與正向控制皆按預期 rc | 29/29 控制通過 |
| Source state (before) | `sh tools/gate/source_state.sh --base main` | 乾淨樹、非 shallow、無 whitelist | `5ef0258…`，exclusions none |
| Tests | `cargo test --all-targets` | 0 new failures vs baseline（baseline 0） | 84 passed（unit）+ 7 passed（integration）= 91，0 failed |
| Types | `cargo check --all-targets` | 0 errors | 0 errors |
| Lint + format | `cargo fmt --check`；`cargo clippy --all-targets -- -D warnings` | 0 warnings | 0 warnings |
| Suite health | `cargo test --all-targets` ×2（平行）+ `-- --test-threads=1` ×1 | 隨機順序 0 flakes | SUBSTITUTED：3 次皆 91 passed；無隨機順序（`--shuffle` 需 nightly） |
| Changed-line coverage | `cargo llvm-cov --all-targets --no-report` + `report --lcov/--json` + `python3 tools/gate/changed_lines.py --allow tools/gate/coverage-allow.txt` | src/ 下每個新增可執行行被執行；0 unmapped；allowlist 每條精確對應 1 行 | 267/270 changed executable lines covered（src/config.rs 262/265、src/main.rs 4/4、src/cachedir.rs 1/1）；300 not executable；**100 executable with no coverage mapping — 全部在 tests/cc_statusline.rs**（cargo-llvm-cov 報表預設排除 tests/ 原始碼；資訊性，不判定）；3 條 allowlist（見 Negative controls 下方逐字引用）。DEPENDENCY UNMET：依賴 suite health，其為 SUBSTITUTED |
| Mutation | `cargo mutants --in-diff .gate/config-file/change.diff --jobs 1 --cap-lints true` | 0 surviving mutants not classified equivalent；0 timeout | 27 mutants：22 caught、0 missed、0 timeout、5 unviable（分類見下）。DEPENDENCY UNMET：依賴 suite health |
| Mutation kill sample | `cargo mutants … -F config_file_path`、`-F read_config_file`（各自新輸出目錄） | 重新施加後 missed == 0 且 caught ≥ 1 | config_file_path 3/3 caught；read_config_file 11/11 caught（14 個 mutant 重新施加，14/14 再次被殺） |
| Real execution | `sh tools/gate/real_execution.sh`（debug binary、隔離 HOME、預植版本快取、空 CLAUDE_CONFIG_DIR、fixture stdin） | dots 檔 → stdout 含 ● 無 ▓、stderr 空；型別錯誤檔 → stdout 含 Fable 5 且為 bar、stderr 含 config.toml；無檔 → bar、stderr 空；三者 exit 0 | 3/3 ok（輸出逐字記於 `.gate/config-file/real-execution.txt`） |
| Must-not scans | `must_not_match` ×4 + `python3 tools/gate/unchanged_tests.py` | 4 個禁用 pattern 在對應範圍 0 命中；既有測試／fixture／版號／lock 約束成立 | 0 命中；`Cargo.lock new crates = ['basic-toml']`；既有測試逐字相同 |
| Supply chain | lock 差異 + `cargo metadata --locked` + `python3 tools/gate/dep_licenses.py` + secrets grep + capability diff | 新 crate 恰為 spec 授權者且 license MIT/Apache；diff 無 secrets pattern | added crate: basic-toml 0.1.10 `MIT OR Apache-2.0`；removed: none；secrets 0 命中；capability diff：產品新增 `use std::fs::File`、`std::io::{Read, Write}`、`std::ffi::OsStr`、`std::path::{Path, PathBuf}`（檔案讀取＋stderr 寫入；無網路、無子程序）；測試新增 `tempfile::TempDir`、`std::os::unix::fs::PermissionsExt`。弱點稽核 UNAVAILABLE |
| Source state (after) | 同 before，並 `cmp` | 與 before 逐字相同 | 相同（`5ef0258…`） |
| Property-based | — | — | N-A（見下） |

Unviable mutants（5，皆為 `Default::default()` 替換而目標型別未實作 `Default`，任何設定下都無法編譯，非 lint 造成；`--cap-lints true` 已排除 deny(warnings) 因素——首次未加時 17/20 unviable，加上後同一組 mutant 15/15 被殺）：

- src/main.rs:189 `StatusLineApp<…>::system -> Self with Default::default()`
- src/config.rs:123 `Config::resolve -> Self with Default::default()`
- src/config.rs:137 `Config::load -> (Self, Option<String>) with (Default::default(), None | Some(String::new()) | Some("xyzzy".into()))`（3 個）

## Negative controls

`tools/gate/selftest.sh`（29 個，final run 全數如預期）：

- run_layer／finish_gate — 缺層被點名且不印綠（rc 1）；完整 manifest 達綠（rc 0，正向）；失敗指令保留 rc 7；未知層 rc 2；重複層 rc 2
- must_not_match — 乾淨檔通過；禁用 pattern 命中 rc 1；不存在路徑 rc 2；空路徑清單 rc 2
- check_versions — 版本漂移 rc 1；檔案缺失 rc 2
- changed_lines.py — 全覆蓋通過（正向）；零命中行 rc 1；span 內無 DA 行以 unmapped rc 1；空 diff rc 2；LCOV 不可讀 rc 2；allowlist 命中未覆蓋行 → 接受（rc 0）；過期 allowlist rc 2
- added_lines.py — 產品行保留、mod tests 行排除；只含測試模組行的 diff rc 2；縮排的 helper `#[cfg(test)]` 不截斷產品區
- unchanged_tests.py — 改動 inline test rc 1；版號 bump rc 1；找不到測試模組 rc 2；main.rs 產品行改動通過（正向）
- source_state.sh — 未列白名單的 untracked 產品檔拒絕；列在白名單但目錄含 tracked 檔仍拒絕；只含白名單 untracked 內容仍發出 state（正向）
- Mutation baseline under load — `--jobs 1`（無平行 job，無共享 build 目錄；cargo-mutants 每個 job 複製整棵樹），未變異 baseline 在 final run 通過（9s build + 0s test）；先前 3 次完整 mutation run 的 caught/unviable 集合完全相同（22/5），layer 為 deterministic
- Mutation kill sample — 2 個函式共 14 個 caught mutant 於獨立輸出目錄重新施加，14/14 再次被殺；樣本涵蓋 22 個 caught 中的 14 個（64%），其餘 8 個未重驗
- Accepted uncovered lines（`tools/gate/coverage-allow.txt`，final run 逐字）：
  - `src/config.rs:101: Err(_) => Err("config file parser thread panicked".to_owned()),` — defensive arm: basic-toml does not panic on any input within the 16 KiB cap; reaching it would require mocking the parser thread (the unit under test), which the gate forbids
  - `src/config.rs:103: Err(error) => Err(error.to_string()),` — defensive arm: std::thread::Builder::spawn fails only on OS thread-resource exhaustion, which no test can induce deterministically without mocking the unit under test
  - `src/config.rs:71: return (FileConfig::default(), Some(diagnostic(path, &error)));` — platform split, gate ran on Windows: read_to_end after a successful open fails only for a directory on Unix (S10 covers it there); on Windows File::open of a directory fails first, so S10 covers the open-error line and this read-error line is unreachable

Classifier rule（changed_lines.py，home-grown）：純空白、`//` 註解、`#[..]` 屬性、`use`/`mod` 項、只含分隔符與 `?`（含 `else`）的行視為不可執行；其餘在函式 region span 內且無 DA 記錄者一律歸入 set 3（unmapped）。`?` 列入分隔符類的代價：多行運算式結尾 `)?;` 的錯誤路徑不由本層判定（本變更產品碼無此類行；測試碼有）。

## Layers not run as specified

- **N-A (this project has no such surface):** Property-based — Tier 2；變更自身邏輯（優先序、夾限、路徑決議）已由 scenario 測試窮舉，唯一 parser 為第三方 basic-toml
- **UNAVAILABLE (tool missing):** 弱點稽核（cargo-audit 未安裝、spec 未授權安裝）；branch coverage（需 nightly `-Z coverage-options=branch`）
- **SUBSTITUTED:** Suite health — 3 次重跑（2 平行 + 1 單執行緒）代替隨機順序；無法偵測整套件的順序相依
- **NOT REACHED:** none（final run 14 層全部執行）
- **DEPENDENCY UNMET:** Changed-line coverage 與 Mutation 皆依賴 suite health 的決定性；該層為 SUBSTITUTED，因此兩者的數字建立在「未以隨機順序證明」的決定性上。實際觀察：本次工作期間全套件跑了 30 次以上，出現 2 個既有測試各 1–2 次的時序性 flake（見 Dismissed concerns），皆非本變更新增或觸及的測試

## Dismissed concerns

- 既有測試 `oauth::tests::returns_a_fresh_usage_cache_without_a_second_request` 於 gate attempt 4 的 suite-health 第 1 次重跑失敗一次、於 replay（commit b9f2e88 tools-only）失敗一次後重跑通過 — dismissed as pre-existing：src/oauth.rs 不在 diff 內（最後修改 7ffc1ca，早於本變更）；該測試以真實檔案 mtime 對照寫入前取得的 `now`，跨秒時 `checked_sub` 得 None 而誤判快取過期。log：scratchpad `gate-run-attempt1-suitehealth-flake.log`
- 既有測試 `gitstatus::tests::returns_stale_cache_when_the_git_command_times_out` 於 replay 中在 2 個 tools-only commit（b410d2f、e478aea）各失敗一次 — dismissed as pre-existing：src/gitstatus.rs 不在 diff 內；timeout 型測試在 CPU 負載（同時進行 mutation build）下逾時
- tests/cc_statusline.rs 100 行「executable with no coverage mapping」 — dismissed：`cargo llvm-cov` 的 `files`／LCOV 報表不含 tests/ 原始碼（final run 的 `coverage.lcov` SF 清單只有 src/），function span 卻來自測試 binary；這些行是測試 harness 本身，由 tests 層執行（7/7 通過）
- code review F1–F5、G1（codex-astra 三輪）— 全部 closed，最後一輪「無」新 finding

## Structural blind spot

- gate 只在 Windows 11（x86_64-msvc）執行：Unix 專屬路徑（目錄 read 錯誤分支、0700 目錄權限檢查、`secret-tool`）與 macOS keychain 路徑未在此 gate 執行；CI matrix（macOS／Ubuntu／Windows）會跑 `cargo test`，但 coverage／mutation 只在本機 Windows 產生
- 隨機測試順序不可得（stable toolchain），整套件順序相依性未被任何工具檢測
- 解析 thread 的 spawn 失敗與 panic 防禦分支永遠不會被測試執行（allowlist）

## Honest notes

- Gate 共 5 次嘗試才到 final run，每次失敗都是 gate 自身的缺陷或既有 flake，未曾為了通過而改測試或實作：(1) versions.txt 經 core.autocrlf 檢出帶 CR → 版本比對改為去 CR；(2) coverage 閘把 tests/ 行納入判定 → 改為只判定 src/；(3) must-not 掃描把測試的暫存檔寫入視為違規 → 改為只掃產品行；(4) unchanged_tests 以檔案第一個 `#[cfg(test)]` 定位測試模組，被 src/main.rs 的 inline helper 屬性誤導 → 改錨定 `mod tests`；(5) suite-health 遇既有 oauth flake → 原樣重跑。每次修正都有對應的 selftest 控制，attempt log 保留於 scratchpad
- 兩次 `git commit` 遇 Windows 上暫時性 `unable to write new index file`，重試即成功；未使用 `--no-verify` 或任何繞過
- 環境變更（spec Setup plan 授權）：`rustup component add llvm-tools-preview`、`cargo install cargo-llvm-cov 0.9.0 --locked`、`cargo install cargo-mutants 27.1.0 --locked`；未加入 `.gitattributes`（改以 CR 容忍處理，避免超出授權的產品路徑寫入）
- 隔離樹 vs 落地樹：gate 在 `../status-line-gate`（detached at 5ef0258）執行；落地樹另有使用者未提交的 `AGENTS.md`、`docs/agents/issue-tracker.md` 修改（純文件，不參與編譯），本報告不涵蓋
- Mutation 首次 run 17/20 unviable 的原因是 `deny(warnings)` 讓留下未用參數的 mutant 無法編譯，等於沒測；加 `--cap-lints true` 後才得到真正的 15/15（後為 22/22）
- 獨立 code review（codex-astra）第 1 輪 high finding 由我實測重現：basic-toml 在 1 MiB 主執行緒於巢狀深度 2500（約 5 KB）stack overflow、程序中止、stdout 為空，違反 Must NOT 第 3 條；此即 spec v3「資源上限」的來源
- 實作由獨立 sonnet 實作者以 RED→GREEN 節奏完成；本報告不採信其敘述，所有結論來自 git 重放與 gate 執行
