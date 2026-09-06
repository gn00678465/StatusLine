# 使用者層級設定檔慣例調查：位置、格式、優先序與容錯

調查日期：2026-09-06
調查動機：cc-statusline 的使用者透過 `chezmoi update` 管理 dotfiles，而 `~/.claude/settings.json` 是 Claude Code 自身的設定檔，每次 `chezmoi apply` 都會被來源狀態整檔覆寫，使用者無法在其中安全地保留本機專屬的環境變數。本調查的目的是找出成熟 CLI 專案（尤其是 Rust 生態與 Claude Code 周邊工具）如何設計「使用者層級設定檔」，作為 cc-statusline 是否要新增獨立設定檔（而非寄生在 `~/.claude/settings.json` 裡）的依據。

---

## 1. 問題與目標

cc-statusline 目前沒有自己的使用者層級設定檔；所有行為都由 Claude Code 呼叫時傳入的 stdin JSON 與少數環境變數決定。若要支援使用者自訂（例如關閉某個區塊、調整顏色、設定 API 逾時），需要決定：設定檔要放在哪個路徑（是否遵循 XDG，Windows／macOS 是否要各自為政）、要用哪種格式（TOML／JSON／KEY=value）、CLI 旗標／環境變數／設定檔／內建預設值之間的優先序為何，以及檔案不存在或格式錯誤時要怎麼辦——這些決策必須讓 render path 仍然「至少印出 `Claude` 並以 exit 0 結束」（AGENTS.md 的既有原則），同時要能被 `chezmoi` 管理而不與 Claude Code 自身的設定檔互相覆寫。以下逐項調查一手來源。

---

## 2. 成熟專案的設定檔位置

### 2.1 XDG Base Directory 規範本身

freedesktop.org 的官方規範定義：「There is a single base directory relative to which user-specific configuration files should be written. This directory is defined by the environment variable `$XDG_CONFIG_HOME`... If `$XDG_CONFIG_HOME` is either not set or empty, a default equal to `$HOME/.config` should be used.」規範只涵蓋 Linux／類 Unix，**沒有**定義 Windows 或 macOS 的對應行為；是否在 Windows／macOS 上比照辦理，完全是各專案自行決定的慣例，不是規範要求。（[freedesktop.org XDG Base Directory Specification](https://specifications.freedesktop.org/basedir/latest/)）

### 2.2 Rust 平台路徑三個候選 crate

| crate | 設計哲學 | Linux config_dir | macOS config_dir | Windows config_dir |
| --- | --- | --- | --- | --- |
| `dirs` | 每平台各自「原生」慣例，不跨平台統一 | `$XDG_CONFIG_HOME` 或 `$HOME/.config`（依 XDG spec） | `$HOME/Library/Application Support`（`config_dir()` 已改為回傳此值，取代舊的 Preferences） | Roaming `AppData`（`config_local_dir()` 另外提供 non-roaming 版本） |
| `directories`（`ProjectDirs`） | 同上，但額外做「專案專屬」子路徑組裝 | `$XDG_CONFIG_HOME/<project_path>` 或 `$HOME/.config/<project_path>`，例：`/home/alice/.config/barapp` | `$HOME/Library/Application Support/<project_path>`，例：`/Users/Alice/Library/Application Support/com.Foo-Corp.Bar-App` | `{FOLDERID_RoamingAppData}\<project_path>\config`，例：`C:\Users\Alice\AppData\Roaming\Foo Corp\Bar App\config` | ([docs.rs/directories ProjectDirs](https://docs.rs/directories/latest/directories/struct.ProjectDirs.html)) |
| `etcetera` | **刻意把選擇權交給呼叫者**：提供 `Xdg`／`Apple`／`Windows` 三種 `BaseStrategy`，呼叫端自己決定套哪一種，而不是像 `dirs`/`directories` 那樣依編譯平台自動選一種 | `XdgBaseStrategy` 實作 XDG spec | `AppleBaseStrategy` 依 Apple File System Basics 慣例 | `WindowsBaseStrategy` 依 Microsoft KnownFolderID | ([docs.rs/etcetera](https://docs.rs/etcetera/latest/etcetera/), [crates.io/etcetera](https://crates.io/crates/etcetera/0.4.0)) |

`dirs`／`directories` 的關鍵差異在於：它們在**每個平台上各自回傳「該平台原生」的路徑**（macOS 用 Application Support、Windows 用 AppData Roaming），並不會讓 XDG 慣例「滲透」到非 Linux 平台。`etcetera` 則是明確為了讓工具作者能「在 macOS/Windows 上也選擇 XDG 慣例」而設計的（許多終端機／編輯器類工具偏好這樣做，見下）。

### 2.3 Rust CLI 工具的實際作法

| 工具 | 語言 | 預設路徑（Linux/macOS） | 預設路徑（Windows） | 環境變數覆寫 | 是否在 Win/macOS 上仍走 XDG |
| --- | --- | --- | --- | --- | --- |
| **starship** | Rust | `~/.config/starship.toml`（macOS 沿用同一 `~/.config`，不用 Application Support） | 同一慣例（`~/.config/starship.toml`，`~` 展開為使用者家目錄） | `STARSHIP_CONFIG`（完整檔案路徑，非目錄） | 是（macOS、Windows 皆用 XDG 風格 `~/.config`，而非平台原生位置） |
| **ripgrep** | Rust | **無預設位置** | 無預設位置 | `RIPGREP_CONFIG_PATH`（必須手動設定，否則完全不讀設定檔） | 不適用（沒有預設路徑可言） |
| **bat** | Rust | 早期用 `dirs` crate 的平台原生路徑（macOS 原本是 `~/Library/Preferences/bat`），[PR #491](https://github.com/sharkdp/bat/pull/491) 之後 macOS 改為 `~/.config/bat/` | `%APPDATA%\bat\config`（`dirs` crate 的 Windows Roaming AppData） | `BAT_CONFIG_PATH`（檔案）、`BAT_CONFIG_DIR`（目錄，優先於 `$XDG_CONFIG_HOME`） | 部分是——bat 團隊特地把 macOS 從「平台原生」改成「XDG 風格」 |
| **delta** | Rust | 沒有獨立設定檔——沿用 `~/.gitconfig`（git 自己的機制）；不支援 `$XDG_CONFIG_HOME/git/config`（[issue #1971](https://github.com/dandavison/delta/issues/1971)），可用 `--config <path>` 指定替代檔案 | 同左 | 無專屬環境變數；靠 git 的機制 | 不適用（借用 git 的設定系統，且該系統本身尚未完整支援 XDG） |
| **zoxide** | Rust | **沒有設定檔**，僅有一批 `_ZO_*` 環境變數（如 `_ZO_DATA_DIR`、`_ZO_MAXAGE`） | 同左 | 全部靠環境變數 | 不適用 |
| **alacritty** | Rust | 依序尋找 `$XDG_CONFIG_HOME/alacritty/alacritty.toml`、`$XDG_CONFIG_HOME/alacritty.toml`、`$HOME/.config/alacritty/alacritty.toml`、`$HOME/.alacritty.toml`、`/etc/alacritty/alacritty.toml` | `%APPDATA%\alacritty\alacritty.toml` | 無專屬環境變數（可用 `--config-file` CLI 旗標） | 否（Windows 用原生 `%APPDATA%`，不強推 XDG） |
| **helix** | Rust | `~/.config/helix/config.toml` | `%AppData%\helix\config.toml` | 無（可用 `:config-open` 開啟） | 否（Windows 明確用 `%AppData%`） |
| **cargo** | Rust | 階層式 `.cargo/config.toml`：從目前目錄逐層往上找，每一層都疊加，最後疊上 `$CARGO_HOME/config.toml`（預設 `$HOME/.cargo/config.toml`） | `$CARGO_HOME/config.toml` 預設 `%USERPROFILE%\.cargo\config.toml` | 每個設定鍵都有對應的 `CARGO_<SECTION>_<KEY>` 環境變數 | 否（`.cargo` 是自訂目錄名，不用 XDG，也不用平台原生設定目錄） |
| **gitui** | Rust | key bindings／theme：`$XDG_CONFIG_HOME/gitui/*.ron`，未設則 Linux 用 `$HOME/.config/gitui/`；文件同時列出 macOS 也用 `$HOME/.config/gitui/` | `%APPDATA%/gitui/*.ron` | 無獨立環境變數（沿用 XDG 變數） | 是（macOS 與 Linux 共用同一份 `.config` 慣例） |
| **lazygit**（Go，作對照組） | Go | Linux：`~/.config/lazygit/config.yml`；macOS：官方文件列出 ``~/Library/Application\ Support/lazygit/config.yml``，但若使用者自行 `export XDG_CONFIG_HOME=$HOME/.config` 則改用 `~/.config/lazygit` | `%LOCALAPPDATA%\lazygit\config.yml`（同時也會檢查 `%APPDATA%\lazygit\config.yml`） | `XDG_CONFIG_HOME` 在所有平台皆生效；亦有 `--use-config-dir`／`--use-config-file` CLI 旗標 | 部分——macOS 預設仍是 Application Support，但**尊重使用者自設的 `XDG_CONFIG_HOME`**，並非「無條件在所有 OS 上都用 XDG」（原題目「XDG on all OS」的說法未能在官方文件中得到完全一致的確認，實際情況是「macOS/Windows 原生預設 + 尊重 `XDG_CONFIG_HOME` 覆寫」） |
| **gh CLI**（Go，作對照組） | Go | `$XDG_CONFIG_HOME/gh`（若設定），否則 `$HOME/.config/gh` | `%AppData%\GitHub CLI`（若 `$AppData` 存在），否則同 Unix 邏輯 fallback 到 `$HOME/.config/gh` | `GH_CONFIG_DIR`（優先於以上所有規則） | 是，且優先序寫得很明確：「the default value will be one of the following paths (in order of precedence): `$XDG_CONFIG_HOME/gh` (if `$XDG_CONFIG_HOME` is set), `$AppData/GitHub CLI` (on Windows if `$AppData` is set), or `$HOME/.config/gh`」 |

引用來源：starship（[Advanced Configuration](https://starship.rs/advanced-config/)、[DeepWiki Config Loading](https://deepwiki.com/starship/starship/4.2-configuration-loading-and-hierarchy)）；ripgrep（[GUIDE.md](https://github.com/BurntSushi/ripgrep/blob/master/GUIDE.md)）；bat（[README](https://github.com/sharkdp/bat/blob/master/README.md)、[PR #491](https://github.com/sharkdp/bat/pull/491)、[issue #2890](https://github.com/sharkdp/bat/issues/2890)）；delta（[configuration.md](https://github.com/dandavison/delta/blob/main/manual/src/configuration.md)、[issue #1971](https://github.com/dandavison/delta/issues/1971)）；zoxide（[README](https://github.com/ajeetdsouza/zoxide/blob/main/README.md)）；alacritty（[config-alacritty.html](https://alacritty.org/config-alacritty.html)）；helix（[docs.helix-editor.com/configuration.html](https://docs.helix-editor.com/configuration.html)）；cargo（[Configuration – The Cargo Book](https://doc.rust-lang.org/cargo/reference/config.html)）；gitui（[KEY_CONFIG.md](https://github.com/gitui-org/gitui/blob/master/KEY_CONFIG.md)、[THEMES.md](https://github.com/gitui-org/gitui/blob/master/THEMES.md)）；lazygit（[lazygit.dev/docs/configuration](https://lazygit.dev/docs/configuration/)、[docs/Config.md](https://github.com/jesseduffield/lazygit/blob/master/docs/Config.md)）；gh CLI（[cli.github.com/manual/gh_help_environment](https://cli.github.com/manual/gh_help_environment)）。

**小結**：Rust CLI 生態並非「一致遵循 XDG」，而是分成兩派——(a) 終端機／提示字元／編輯器類工具（starship、alacritty on Unix、helix on Unix、gitui）傾向用 XDG 風格 `~/.config`，Windows 上則老實用 `%APPDATA%`；(b) 建構工具類（cargo）用自訂目錄名（`.cargo`）而非通用設定目錄，且强調「階層式探測 + 環境變數覆寫」而非單一固定路徑。沒有任何一個調查到的 Rust 工具在 **Windows** 上把 XDG 變數（`XDG_CONFIG_HOME`）當一等公民；lazygit（Go）是唯一一個明確表示「尊重使用者自設的 `XDG_CONFIG_HOME`」的對照組，但即使如此，它 macOS/Windows 的**預設值**仍是平台原生位置，不是 `~/.config`。

---

## 3. Claude Code 生態的作法

### 3.1 Claude Code 本身

官方文件明確定義：「On Windows, `~/.claude` resolves to `%USERPROFILE%\.claude`. If you set `CLAUDE_CONFIG_DIR`, every `~/.claude` path on this page lives under that directory instead.」（[Explore the .claude directory](https://code.claude.com/docs/en/claude-directory)）也就是說：

- 預設目錄：`~/.claude`（Linux/macOS 為 `$HOME/.claude`，Windows 為 `%USERPROFILE%\.claude`）——**不是** XDG 風格的 `~/.config/claude`，也不是 macOS 的 `~/Library/Application Support`。
- 環境變數覆寫：`CLAUDE_CONFIG_DIR`，可設為單一目錄，設定後連 session 歷史、外掛都會整批搬過去。
- 值得注意的落差：截至調查時，`CLAUDE_CONFIG_DIR` 的說明頁面連結指向 `/docs/en/env-vars`，但實際去抓 `env-vars` 頁面內文，逐字搜尋 `CLAUDE_CONFIG_DIR` **完全沒有出現**——這個變數目前只在 `.claude` 目錄說明頁被提及，官方環境變數總表反而漏了它。這點在 2026 年較早之前甚至完全沒寫進任何官方文件，只能從社群回報的 [issue #33430](https://github.com/anthropics/claude-code/issues/33430) 得知其存在與用法（該 issue 原文抱怨：「this is not documented anywhere — not in `claude --help`, not in the official docs」）。

### 3.2 statusline 專案：ccstatusline（sirmalloc）

原始碼 `src/utils/config.ts` 對設定檔路徑的計算方式是：

```javascript
const DEFAULT_SETTINGS_PATH = path.join(os.homedir(), '.config', 'ccstatusline', 'settings.json');
```

也就是**跨平台一律用同一條路徑規則**：`os.homedir()` 取得家目錄後，無條件接上 `.config/ccstatusline/settings.json`，完全不檢查 `XDG_CONFIG_HOME`，也不因應 Windows 改走 `%APPDATA%`。結果是：

- Linux/macOS：`~/.config/ccstatusline/settings.json`
- Windows：`C:\Users\<user>\.config\ccstatusline\settings.json`（在 `%USERPROFILE%` 下手動建一個 `.config` 資料夾，而非用 Windows 原生的 `%APPDATA%`）

（[sirmalloc/ccstatusline `src/utils/config.ts`](https://github.com/sirmalloc/ccstatusline/blob/main/src/utils/config.ts)）

需要特別區分：ccstatusline 專案文件（`docs/WINDOWS.md`）裡另外提到的 `%USERPROFILE%\.claude\settings.json` 與 `CLAUDE_CONFIG_DIR`，講的是 **Claude Code 自身**用來呼叫 statusline 指令的 `settings.json`（即 `{"statusLine": {"command": "ccstatusline"}}` 這一段設定所在的檔案），跟 ccstatusline **自己的**外觀設定檔（上面的 `~/.config/ccstatusline/settings.json`）是兩個不同的檔案、不同的路徑規則。這正好示範了本次調查動機所指出的問題：一個 statusline 工具最終要處理「兩層」設定——Claude Code 用來啟動它的那份，以及它自己的偏好設定。（[docs/WINDOWS.md](https://github.com/sirmalloc/ccstatusline/blob/main/docs/WINDOWS.md)）

格式：JSON（`settings.json`），內容是一個 `items` 陣列，每個元素帶 `type`／`color` 等欄位；ccstatusline 附一個互動式 TUI（terminal UI wizard）來產生/編輯這份 JSON，而不要求使用者手動編輯。

### 3.3 Claude Code 生態的統整表

| 專案 | 設定檔位置 | 格式 | XDG？ | 備註 |
| --- | --- | --- | --- | --- |
| Claude Code 本身 | `~/.claude/`（Win: `%USERPROFILE%\.claude`） | JSON（`settings.json`） | 否 | 可用 `CLAUDE_CONFIG_DIR` 整批搬遷；該變數目前只出現在 `.claude` 目錄文件頁，未列入官方 env-vars 總表 |
| ccstatusline（sirmalloc） | `~/.config/ccstatusline/settings.json`（跨平台同一規則，Windows 也是 `.config` 而非 `%APPDATA%`） | JSON | 半是——形式上像 XDG 路徑，但**未實際讀取 `XDG_CONFIG_HOME`**，是寫死的字串 | 附 TUI wizard 產生設定 |

（未能取得 ccusage statusline、claude-hud 對其設定檔位置與格式的一手原始碼／README 佐證，故不列入此表以免臆測；若需要可再指定專案深入查證。）

---

## 4. 格式取捨

### 4.1 TOML／JSON／JSONC／YAML／KEY=value 的取捨

| 格式 | 註解支援 | 人工編輯友善度 | 對 cc-statusline 的新增依賴成本 |
| --- | --- | --- | --- |
| TOML | 原生支援 `#` 註解 | 高（starship、cargo、alacritty、helix 都選 TOML） | 需引入 `toml` crate（見 4.2） |
| JSON | **不支援**註解（見 4.3） | 中（機器產生友善，人工維護時無法加說明） | 零額外依賴——`serde_json` 已在專案裡 |
| JSONC | 支援 `//`／`/* */` 註解，但不是 RFC 8259 標準 JSON，需要額外解析器或預處理 | 高 | 需要額外 crate（`serde_json` 官方不支援，見 4.3） |
| YAML | 支援 `#` 註解 | 中高，但縮排敏感、多義性高（如 `on`/`off` 隱式布林），公認對人工編輯較不安全 | 需要 `serde_yaml` 等 crate，且 YAML parser 一般比 TOML parser 依賴更重 |
| KEY=value（dotenv/INI 風格） | 依實作而定，ripgrep 的「每行一個參數 + `#` 註解」風格屬於此類 | 最低學習成本，但不支援巢狀結構 | 可用 `std::fs::read_to_string` + 手寫逐行解析，零依賴 |

Rust CLI 生態裡選 TOML 的工具（starship、cargo、alacritty、helix）都是「设定项目不深、需要人工維護註解」的場景，這與 cc-statusline 的使用情境相符。

### 4.2 `toml` crate 的實際依賴成本（2026-09 現況，crates.io 實測版本）

toml 生態這幾年做過一次大重構：舊版 `toml` crate（2.x 之前）幾乎整包委派給 `toml_edit`（保留格式的編輯器），而 `toml_edit` 又依賴 `winnow`（parser combinator）、`serde_spanned`、`toml_datetime`、`indexmap`。但**目前**（`toml` 1.1.5+spec-1.1.0，2026-09-02 發布）的架構已經拆得更細，`toml` crate 的一般（serde 反序列化）路徑依賴為：

- `serde_spanned` `^1.1.1`
- `toml_datetime` `^1.1.1`
- `toml_parser` `^1.1.3`（optional，parsing 用；本身依賴 `winnow ^1.0.0`）
- `toml_writer` `^1.1.2`（optional，序列化用；本身幾乎無執行期依賴）
- `serde_core` `^1.0.228`（optional）
- `indexmap` `^2.13.0`（optional，通常在啟用 `preserve_order` 一類 feature 才需要）

（[docs.rs/crate/toml/latest](https://docs.rs/crate/toml/latest)、[docs.rs/crate/toml_parser/latest](https://docs.rs/crate/toml_parser/latest)、[docs.rs/crate/toml_writer/latest](https://docs.rs/crate/toml_writer/latest)）

對照組 `toml_edit`（0.25.13+spec-1.1.0，2026-07-14）依賴 `winnow ^1.0.0`、`serde_spanned ^1.1.1`、`toml_datetime ^1.1.1`、`indexmap ^2.13.0`——這是「保留格式編輯」（round-trip 保留註解、空白、順序）情境才需要的較重路徑；純讀取設定用不到 `toml_edit`。（[docs.rs/crate/toml_edit/latest](https://docs.rs/crate/toml_edit/latest)）

**對 cc-statusline 的實際意義**：若只需要「讀取設定檔、反序列化成 struct」（不需要保留註解回寫），`toml = { version = "1", default-features = false, features = ["parse"] }` 這類最小組合，理論上只多帶進 `toml_parser`（→ `winnow`）、`toml_datetime`、`serde_spanned` 三～四個小 crate，不需要 `toml_edit`、`indexmap`。這比舊架構印象中的「toml 一定拖 toml_edit + winnow + indexmap」要輕。

### 4.3 更輕量的替代：`basic-toml`

`basic-toml` 的定位是：「a stripped down fork of version 0.5 of the `toml` crate (from before the `toml_edit` rewrite)... a minimal TOML library with few dependencies」（[crates.io/basic-toml](https://crates.io/crates/basic-toml)）。它只做「parse 成 serde 資料結構」這一件事，不支援保留格式編輯、不支援寫回帶註解的 TOML，換取更小的依賴樹。對 cc-statusline 這種「只讀不寫」的設定檔情境，是比 `toml`/`toml_edit` 更貼近「近零依賴」原則的選項；`toml-parse` 是另一個同類定位的輕量 parser，但本次未能取得其目前的一手 crates.io 依賴清單，未能自一手來源確認其具體版本與依賴數，故不列入比較表。

### 4.4 `serde_json` 對「寬鬆解析」的支援：不支援註解

`serde_json` 官方 crate 文件（docs.rs 首頁說明）未見任何允許註解／trailing comma 的敘述；社群多次要求加入註解支援，均維持「serde_json 只做嚴格 JSON」的立場，可見多個長期開放或被引導至替代方案的 issue：[serde-rs/json#168 "Allow comments in JSON"](https://github.com/serde-rs/json/issues/168)（2016 年開的最早一批請求之一）、[serde-rs/json#394 "JSON5 support"](https://github.com/serde-rs/json/issues/394)、[serde-rs/json#1196 "Support for Comments"](https://github.com/serde-rs/json/issues/1196)。本次未能取得 dtolnay 在該串討論中逐字的回覆內容（GitHub 頁面內容抓取受限），因此「維持嚴格 JSON」是根據 issue 長期未被實作、且社群普遍轉向 `json5`／`serde-hjson`／`serde_jsonc`（第三方 fork）等替代 crate 的事實推斷，而非直接引用維護者聲明——**未能自一手來源確認 dtolnay 的逐字說法**，僅能確認的一手事實是：截至本次調查，`serde_json` 本身不解析含 `//` 或 `/* */` 註解的 JSON。

**結論**：若要在 JSON 格式裡保留「給人看的註解」，`serde_json` 做不到；要嘛換 TOML（原生支援），要嘛接受 JSON 不能有註解的限制。

---

## 5. 優先序慣例

### 5.1 clig.dev（Command Line Interface Guidelines）的明文建議

clig.dev「Configuration」一節原文（逐字引用）：

> "Apply configuration parameters in order of precedence. Here is the precedence for config parameters, from highest to lowest:
> - Flags
> - The running shell's environment variables
> - Project-level configuration (e.g. `.env`)
> - User-level configuration
> - System wide configuration"

（[clig.dev](https://clig.dev/)）

### 5.2 cargo 的實作與文件

Cargo 官方文件明確聲明：「Configuration values specified this way take precedence over environment variables, which take precedence over configuration files.」且配置檔本身走「階層探測、就近優先」規則——從目前目錄逐層往上找 `.cargo/config.toml`，每層都疊加，越接近目前目錄的檔案優先權越高，家目錄（`$CARGO_HOME`）優先權最低；命令列 `--config` 覆寫最上層。完整優先序（高到低）：`--config` CLI 旗標 → `CARGO_*` 環境變數 → 階層式 `.cargo/config.toml`（近覆遠）→ 內建預設值。（[The Cargo Book – Configuration](https://doc.rust-lang.org/cargo/reference/config.html)）

### 5.3 starship／ripgrep 的實作

- starship：命令列旗標與 `STARSHIP_CONFIG` 環境變數可覆寫預設檔案路徑本身（而非逐項覆寫設定值），設定檔內容則是唯一的行為來源，模組層級沒有再額外的「CLI 覆寫某個設定鍵」機制。（[Advanced Configuration](https://starship.rs/advanced-config/)）
- ripgrep：明文「All you need to do is pass command-line flags (e.g., `--max-columns 0`) on the command line, which will override your configuration file's setting.」——CLI 旗標覆寫設定檔，設定檔內容等同「預先展開的命令列參數」，二者共用同一套解析規則。（[GUIDE.md](https://github.com/BurntSushi/ripgrep/blob/master/GUIDE.md)）

**共同模式**：CLI 旗標優先權最高、環境變數次之、設定檔再次之、內建預設值最低——這與 clig.dev 的建議、cargo 的文件用語完全一致，可視為業界共識，而非單一專案的個別選擇。

---

## 6. 缺失與格式錯誤時的容錯

| 工具 | 檔案不存在時 | 檔案存在但格式錯誤時 | 一手來源 |
| --- | --- | --- | --- |
| starship | 靜默視為無設定，走全部預設值；`read_config_content_as_str()` 對 `ErrorKind::NotFound` 只記 `log::Debug` 等級 | **不會中止**，`config_from_file()` 對 `toml::from_str` 失敗的處理是 `log::error!("Unable to parse the config file: {error}")` 後回傳 `None`，呼叫端 `initialize()` 接著使用預設的 `StarshipConfig`——即「印警告到 stderr（透過 log crate），但提示字元照常算圖並印出」 | starship 原始碼 `src/config.rs`：<br>`fn config_from_file(...) { ... match toml::from_str(&toml_content) { Ok(parsed) => { log::debug!(...); Some(parsed) } Err(error) => { log::error!("Unable to parse the config file: {error}"); None } } }`；缺檔分支：`Err(e) => { let level = if e.kind() == ErrorKind::NotFound { log::Level::Debug } else { log::Level::Error }; log::log!(level, "Unable to read config file content: {e}"); None }`（[github.com/starship/starship src/config.rs](https://github.com/starship/starship/blob/master/src/config.rs)） |
| ripgrep | 沒有預設路徑，`RIPGREP_CONFIG_PATH` 未設就完全不找檔案，不算「缺失」而是「本來就不啟用」 | 未能自一手來源（GUIDE.md）確認錯誤格式時的具體行為（是否印出解析錯誤訊息、是否中止），GUIDE.md 只描述語法規則與 `--debug`/`--no-config` 除錯手段，未涵蓋錯誤處理路徑 | [GUIDE.md](https://github.com/BurntSushi/ripgrep/blob/master/GUIDE.md) |
| bat | 未能自一手來源確認具體行為（README 僅描述路徑解析與 `--generate-config-file`） | 同上，未能自一手來源確認 | [README](https://github.com/sharkdp/bat/blob/master/README.md) |

**Claude Code statusline 這一類工具的正確行為（推論，非一手來源直接陳述）**：由於 statusline 指令的 stdout **就是**要顯示在使用者介面上的那一行文字，AGENTS.md 既有原則「render path 必須至少印出 `Claude` 並以 exit 0 結束」與 starship 的策略完全同構——這正是 starship 作為「prompt 產生器」被設計成「設定壞掉也絕不能讓 shell prompt 消失」的同一類問題。因此可比照 starship 的模式：

1. 檔案不存在 → 視為「未設定」，靜默套用全部內建預設值，不印任何訊息（避免每次啟動都在 stderr 洗版）。
2. 檔案存在但解析失敗（TOML/JSON 語法錯誤、型別不符）→ 把錯誤訊息寫到 **stderr**（不能寫進 stdout，否則會混進 statusline 本身的輸出），然後照樣套用全部內建預設值，繼續往下 render，最終仍要 exit 0。
3. 唯一不可接受的結果是：因為設定檔問題而 panic、非零 exit，或讓 stdout 為空——這會讓整條 statusline 從 Claude Code 介面上消失。

---

## 7. near-zero-dep Rust 的實作要點

### 7.1 `serde` struct 慣例：`#[serde(default)]` 與 unknown-field 政策

- 每個欄位／整個 struct 標 `#[serde(default)]`，讓「設定檔缺少某個鍵」自動落回 `Default::default()`，而不是解析失敗——這樣新版加欄位不會讓舊設定檔突然壞掉（前向相容）。
- unknown-field 政策：`#[serde(deny_unknown_fields)]` 會讓「設定檔有一個當前版本不認識的鍵」直接解析失敗，優點是能抓到使用者的拼字錯誤；但代價是「舊版 binary 讀新版設定檔（例如降版本、或多機器版本不同步）」會直接壞掉。反過來，預設（不加這個 attribute）是「忽略不認識的鍵」，對「保留設定檔給未來版本、也能被舊版本忽略新增選項」的情境更寬容。取捨屬於專案判斷：若設定檔會被 `chezmoi` 這類工具在多機器間同步、且各機器安裝的 cc-statusline 版本可能不同步，**忽略未知欄位**（不加 `deny_unknown_fields`）比較不容易在版本落差時整檔失效。

### 7.2 讀檔慣例：`NotFound` 視為「無設定檔」，其餘一律「忽略此檔」

慣用模式（對應 starship 的 `read_config_content_as_str` 分支邏輯，見 §6）：

```rust
match std::fs::read_to_string(&path) {
    Ok(s) => /* 解析 s，解析失敗也視為「忽略此檔，用預設值」 */,
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => /* 視為未設定，用預設值 */,
    Err(_) => /* 例如權限不足、是個目錄等——同樣「忽略此檔」而非中止 */,
}
```

這與 AGENTS.md「render path 必須永遠印出東西並 exit 0」的原則一致：任何檔案 I/O 或解析錯誤都不應該讓程式提前結束，一律退回內建預設值。

### 7.3 不依賴 `dirs` crate 的家目錄解析

最小作法是直接讀環境變數：Unix 讀 `HOME`，Windows 讀 `USERPROFILE`（`dirs`/`directories` 內部也是這樣取得家目錄後再組路徑，只是額外處理了 Windows Known Folder API 等 edge case）。

`std::env::home_dir()` 的狀態值得特別澄清，因為調查題目原先假設「Rust 1.85+ 已經解除棄用」，但精確的一手時間線是**兩個不同版本**：

- **Rust 1.29.0**：`std::env::home_dir()` 被標記為 deprecated，原因是它在 Windows 上「若 `HOME` 環境變數被設定（Windows 上這不是標準配置），會回傳令人意外的結果」。
- **Rust 1.85.0**（[PR #132515 "Fix and undeprecate home_dir()"](https://github.com/rust-lang/rust/pull/132515)）：**修正了實作**，讓它在 Windows 上不再理會非標準的 `HOME` 變數，改用正確的 Windows API；但這個 PR 本身**沒有**同時移除 deprecated 標記。
- **Rust 1.87.0**（[PR #137327 "Undeprecate env::home_dir"](https://github.com/rust-lang/rust/pull/137327)）：根據 libs-api 團隊意見，在下一個版本才正式移除 deprecated 標記。

也就是說，**行為修正在 1.85，取消棄用標記在 1.87**——若專案的 MSRV 落在 1.85～1.86 之間，`std::env::home_dir()` 的行為已經正確，但編譯器仍會印出 deprecation warning；本專案 `Cargo.toml` 設了 `[lints.rust] warnings = "deny"`，代表若要在這個版本區間使用 `std::env::home_dir()`，會直接讓建置失敗——除非 MSRV ≥ 1.87，或改用手動讀取 `HOME`/`USERPROFILE` 環境變數（後者也更貼近「近零依賴」與「明確控制」的專案風格，不依賴標準庫這段有爭議歷史的 API）。（[docs.rs std::env::home_dir](https://doc.rust-lang.org/std/env/fn.home_dir.html)、[rust-lang/rust#132650 Tracking issue](https://github.com/rust-lang/rust/issues/132650)、[rust-lang/rust#137866 Tracking issue](https://github.com/rust-lang/rust/issues/137866)）

---

## 8. 與 chezmoi 的互動

chezmoi 的「來源狀態」（source state）用檔名前綴／後綴編碼目標檔案的屬性，官方文件列出的屬性前綴順序為 `run_, exact_, private_, empty_, executable_, symlink_, once_, dot_`，其中最基本的 `dot_` 前綴定義是：「Rename to use a leading dot, e.g. `dot_foo` becomes `.foo`」。（[chezmoi Source state attributes](https://www.chezmoi.io/reference/source-state-attributes/)）

這代表：**家目錄下任何路徑**（不限於頂層的一個點檔案）都能被 chezmoi 管理成鏡射的目錄結構，只要每一層目錄都套上對應前綴。官方文件給的巢狀範例是管理 `~/.config/Code/User/settings.json`：來源狀態目錄結構為 `private_dot_config/private_Code/User/settings.json.tmpl`（`.config` 與 `.config/Code` 都用 `private_` 前綴是因為這兩層目錄預設權限被收斂），也就是 `private_dot_config` 對應 `~/.config`、底下再疊 `private_Code`、`User` 這兩層目錄，最底層才是真正的檔案。

**對 cc-statusline 的意涵**：只要新設定檔選定一個固定路徑（例如 `~/.config/cc-statusline/config.toml`），使用者就能在自己的 chezmoi 來源倉庫裡建立對應的 `dot_config/cc-statusline/config.toml`（或 `private_dot_config/...`，視乎目錄權限需求），把它交給 chezmoi 版本控管；而**不必**再去跟 `~/.claude/settings.json` 共用同一個檔案——這正是題目動機所指出的問題的直接解法：只要 cc-statusline 的設定「不寄生」在 Claude Code 自己的 `settings.json` 裡，`chezmoi update` 重新套用 `~/.claude/settings.json`（Claude Code 自己管的檔案）就不會動到 cc-statusline 的獨立設定檔，兩者互不干擾，各自的 chezmoi 來源檔案分開維護。

---

## 9. 對 cc-statusline 的設計建議

以下為判斷／建議，非全部有一手來源直接背書，會標明依據性質。

### 9.1 路徑候選（排序）

1. **`~/.config/cc-statusline/config.toml`（跨平台統一，比照 starship／gitui／ccstatusline 自身的做法）**——〔判斷〕優點是與同類工具（starship、ccstatusline）一致，使用者心智負擔低，且天然適合被 chezmoi 用 `dot_config/cc-statusline/config.toml` 管理；缺點是在 Windows／macOS 上不是「平台原生」路徑，需要專案自行決定要不要跟隨 `XDG_CONFIG_HOME`（若跟隨，行為會像 starship／bat；若寫死 `.config`，行為會像 ccstatusline 目前的實作）。
2. **平台原生（`dirs`/`directories` 風格：Linux `~/.config`、macOS `~/Library/Application Support`、Windows `%APPDATA%`）**——〔判斷〕更「正統」但需要引入 `dirs`/`directories`/`etcetera` 之一，與「近零依賴」原則衝突；且 cc-statusline 的既有慣例（`~/.claude/cc-statusline/` 存放 binary，見 npm-allow-scripts.md 調查）已經是「不分平台、統一路徑」風格，選項 1 與既有慣例更一致。
3. **寄生在 `~/.claude/cc-statusline/` 目錄下**（binary 已經裝在這裡）——〔判斷〕與現有安裝路徑同目錄，不需要另外決定「設定放哪」，但這個目錄的所有權模糊（是 cc-statusline 的，還是 Claude Code 的子目錄），且若日後改用 npm `optionalDependencies` 模式安裝（見 npm-allow-scripts.md §1.2 建議），這個目錄可能不再穩定存在。**不建議**優先採用。

綜合判斷：建議選項 1，路徑寫死為 `~/.config/cc-statusline/config.toml`（不隨 `XDG_CONFIG_HOME` 變動，理由是 cc-statusline 定位更接近 starship／ccstatusline 這類「單一設定檔、路徑固定好記」的輕量 CLI 附屬工具，而非需要精確遵循 XDG spec 的桌面應用）；若使用者明確要求遵循 `XDG_CONFIG_HOME`，可視為未來的擴充項，而非首版必要條件。

### 9.2 建議格式：TOML

〔判斷，基於 §4 的一手事實〕理由：(a) 支援註解，符合「使用者手動編輯、需要說明每個選項用途」的情境；(b) `serde_json` 官方確認不支援註解（一手事實，§4.4）；(c) 若只需要「讀取、反序列化」不需要「保留格式回寫」，最小化 `toml` crate feature 組合（或改用 `basic-toml`）對「近零依賴」原則的衝擊比預期小（§4.2、§4.3 的一手版本與依賴數字）。若專案評審認為連 `toml`/`basic-toml` 都不想加，退而求其次可採用 ripgrep 式的 `KEY=value` 逐行格式（零依賴，`std::fs::read_to_string` + 手寫 parser），但犧牲巢狀結構表達力。

### 9.3 建議優先序

〔判斷，基於 §5 的業界共識，非 cc-statusline 特有一手來源〕CLI 旗標（如有）> 環境變數 > `~/.config/cc-statusline/config.toml` > 內建預設值——與 clig.dev、cargo、starship、ripgrep 的共同慣例一致。

### 9.4 建議錯誤容錯策略

〔判斷，基於 §6 starship 模式類比，直接呼應 AGENTS.md 既有原則〕：

- 缺檔 → 靜默套用預設值，不印任何訊息。
- 格式錯誤（TOML 語法錯誤、`serde` 型別不符）→ 錯誤訊息寫 stderr（絕不能污染 stdout），套用預設值繼續 render，**必須 exit 0**。
- 不對未知欄位使用 `deny_unknown_fields`（§7.1 的判斷：`chezmoi` 跨機器同步時版本可能不一致，寬容比嚴格更安全）。
- 家目錄解析不必引入 `dirs` crate：直接讀 `HOME`（Unix）／`USERPROFILE`（Windows）環境變數；若考慮 `std::env::home_dir()`，需先確認專案 MSRV ≥ 1.87（§7.3 的一手版本時間線），否則會撞上 `warnings = "deny"` 的 lint 設定。

---

## 10. 來源

**XDG 與 Rust 平台路徑 crate**

- [freedesktop.org — XDG Base Directory Specification](https://specifications.freedesktop.org/basedir/latest/)
- [docs.rs — dirs crate](https://docs.rs/dirs/latest/dirs/)
- [docs.rs — directories crate, `ProjectDirs`](https://docs.rs/directories/latest/directories/struct.ProjectDirs.html)
- [docs.rs — etcetera crate, `base_strategy`](https://docs.rs/etcetera/latest/etcetera/base_strategy/index.html)
- [crates.io — etcetera 0.4.0](https://crates.io/crates/etcetera/0.4.0)

**Rust CLI 工具設定檔**

- [starship — Advanced Configuration](https://starship.rs/advanced-config/)
- [starship — DeepWiki: Configuration Loading and Hierarchy](https://deepwiki.com/starship/starship/4.2-configuration-loading-and-hierarchy)
- [starship 原始碼 — src/config.rs](https://github.com/starship/starship/blob/master/src/config.rs)
- [ripgrep — GUIDE.md](https://github.com/BurntSushi/ripgrep/blob/master/GUIDE.md)
- [bat — README.md](https://github.com/sharkdp/bat/blob/master/README.md)
- [bat — PR #491 "Updated bat config dir for MacOs to ~/.config/bat/"](https://github.com/sharkdp/bat/pull/491)
- [bat — issue #2890 "Include Information on BAT_CONFIG_DIR"](https://github.com/sharkdp/bat/issues/2890)
- [delta — manual/src/configuration.md](https://github.com/dandavison/delta/blob/main/manual/src/configuration.md)
- [delta — issue #1971 "$XDG_CONFIG_HOME/git/config is not checked"](https://github.com/dandavison/delta/issues/1971)
- [zoxide — README.md](https://github.com/ajeetdsouza/zoxide/blob/main/README.md)
- [alacritty — config-alacritty.html](https://alacritty.org/config-alacritty.html)
- [helix — docs.helix-editor.com/configuration.html](https://docs.helix-editor.com/configuration.html)
- [The Cargo Book — Configuration](https://doc.rust-lang.org/cargo/reference/config.html)
- [gitui — KEY_CONFIG.md](https://github.com/gitui-org/gitui/blob/master/KEY_CONFIG.md)
- [gitui — THEMES.md](https://github.com/gitui-org/gitui/blob/master/THEMES.md)
- [lazygit — Configuration (lazygit.dev)](https://lazygit.dev/docs/configuration/)
- [lazygit — docs/Config.md](https://github.com/jesseduffield/lazygit/blob/master/docs/Config.md)
- [GitHub CLI — gh_help_environment](https://cli.github.com/manual/gh_help_environment)

**Claude Code 生態**

- [Claude Code Docs — Explore the .claude directory](https://code.claude.com/docs/en/claude-directory)
- [Claude Code Docs — Environment variables](https://code.claude.com/docs/en/env-vars)
- [anthropics/claude-code — issue #33430（CLAUDE_CONFIG_DIR 未被文件化的原始回報）](https://github.com/anthropics/claude-code/issues/33430)
- [sirmalloc/ccstatusline — 原始碼 src/utils/config.ts](https://github.com/sirmalloc/ccstatusline/blob/main/src/utils/config.ts)
- [sirmalloc/ccstatusline — docs/WINDOWS.md](https://github.com/sirmalloc/ccstatusline/blob/main/docs/WINDOWS.md)

**格式與 crate 依賴**

- [docs.rs/crate/toml/latest](https://docs.rs/crate/toml/latest)
- [docs.rs/crate/toml_edit/latest](https://docs.rs/crate/toml_edit/latest)
- [docs.rs/crate/toml_parser/latest](https://docs.rs/crate/toml_parser/latest)
- [docs.rs/crate/toml_writer/latest](https://docs.rs/crate/toml_writer/latest)
- [crates.io — basic-toml](https://crates.io/crates/basic-toml)
- [docs.rs — serde_json](https://docs.rs/serde_json/latest/serde_json/)
- [serde-rs/json — issue #168 "Allow comments in JSON"](https://github.com/serde-rs/json/issues/168)
- [serde-rs/json — issue #394 "JSON5 support"](https://github.com/serde-rs/json/issues/394)
- [serde-rs/json — issue #1196 "Support for Comments"](https://github.com/serde-rs/json/issues/1196)

**優先序慣例**

- [clig.dev — Command Line Interface Guidelines](https://clig.dev/)

**std::env::home_dir 歷史**

- [doc.rust-lang.org — std::env::home_dir](https://doc.rust-lang.org/std/env/fn.home_dir.html)
- [rust-lang/rust — PR #132515 "Fix and undeprecate home_dir()"](https://github.com/rust-lang/rust/pull/132515)
- [rust-lang/rust — issue #132650 Tracking issue for release notes of #132515](https://github.com/rust-lang/rust/issues/132650)
- [rust-lang/rust — PR #137327 "Undeprecate env::home_dir"](https://github.com/rust-lang/rust/pull/137327)
- [rust-lang/rust — issue #137866 Tracking issue for release notes of #137327](https://github.com/rust-lang/rust/issues/137866)

**chezmoi**

- [chezmoi — Source state attributes](https://www.chezmoi.io/reference/source-state-attributes/)

**本機檔案**（作為既有慣例對照，非外部一手來源）

- `D:\Projects\status-line-dev\status-line\Cargo.toml`（確認 `edition = "2021"`、`[lints.rust] warnings = "deny"`）
- `D:\Projects\status-line-dev\status-line\docs\research\npm-allow-scripts.md`（既有安裝路徑 `~/.claude/cc-statusline/` 的調查依據）
