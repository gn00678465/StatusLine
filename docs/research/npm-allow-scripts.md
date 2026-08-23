# npm `allow-scripts` / install-scripts 政策對 cc-statusline npm 安裝器的影響

調查日期：2026-08-23
調查對象：cc-statusline v2.0.0 的 npm 安裝路徑（`npm/package.json`、`npm/postinstall.js`、`README.md`）
驗證環境：macOS 26.5、Node v26.7.0、npm 11.19.0（mise 管理）、另於沙箱安裝 npm 10.9.9 與 npm 12.0.2 實測
§6 追加驗證環境：pnpm 10.32.0、pnpm 11.22.0、bun 1.4.0、yarn classic 1.22.22、yarn berry 4.13.0 / 4.14.0 / 4.18.0（皆由 mise 安裝，2026-08-23 實測）

---

## 1. 摘要（結論與建議）

### 1.1 三個必須先講清楚的結論

**結論 A：目前看到的只是警告，但 npm 12 會直接讓現在的安裝指令「完全失敗」。**

npm 11.16.0 起是「advisory（警告但仍執行）」階段，npm 12.0.0 起改為「預設封鎖」。而且 npm 12 對本專案的殺傷力有**兩道閘門**，第二道比 `allow-scripts` 更早觸發、也更致命：

| 閘門 | 設定 | npm 11 預設 | npm 12 預設 | 對本專案的後果 |
| --- | --- | --- | --- | --- |
| 1. 遠端 tarball 來源 | `allow-remote` | `all` | `none` | `npm install -g <tgz URL>` 直接 `EALLOWREMOTE` 失敗，連下載都不會發生 |
| 2. 依賴 install script | `allowScripts` / `allow-scripts` | 警告但執行 | 封鎖 | postinstall 被跳過，裝出一個「沒有 binary 的空套件」 |

實測 npm 12.0.2 跑目前 README 的一行指令，結果是：

```
npm error code EALLOWREMOTE
npm error Fetching packages of type "remote" have been disabled
npm error Refusing to fetch "http://127.0.0.1:8750/latest/download/dummy.tgz"
```

**結論 B（本次最重要的發現）：npm 警告訊息裡建議的那個指令是錯的，對本專案無效。**

npm 建議使用者執行：

```
npm install -g --allow-scripts=@gn00678465/cc-statusline
```

但 `@gn00678465/cc-statusline` 這個**套件名字根本不會 match**，因為從 tarball URL 安裝的套件在 npm 的政策比對器裡屬於 `remote` 型別，只能用**完整 resolved URL 字串**精確比對，不能用套件名。實測（npm 11.19.0 `--strict-allow-scripts` 與 npm 12.0.2 皆同）：

| 測試 | `--allow-scripts` 的值 | 結果 |
| --- | --- | --- |
| B | `@testscope/dummy-installer`（npm 自己建議的名字） | **無效**，警告仍在／strict 下安裝失敗 |
| C | `http://127.0.0.1:8750/testscope-dummy-installer-1.0.0.tgz`（完整 URL） | **有效**，警告消失 |

也就是說，「照 npm 的提示做」的使用者會發現問題沒解決。這是 npm CLI 的一個瑕疵（訊息用 display name 產生，但比對用 resolved URL），詳見 §2.4。

**結論 C：GitHub 的 `releases/latest/download/...` 轉址不影響設定值，allow key 可以寫死。**

npm 記錄的 `resolved` 是**使用者輸入的原始 URL**，不是 302 轉址後的最終 URL。實測用 `/latest/download/dummy.tgz`（302 → 帶版號的實際檔案）安裝，只有拿**原始 `latest/download` URL** 當 key 才 match，拿轉址後的最終 URL 當 key 反而失敗。這對本專案是好消息：`.../releases/latest/download/cc-statusline-npm.tgz` 不含版號，所以 allow key 跨版本穩定，不必每次發版就改。

### 1.2 建議（排序）

**第一優先（立即、零風險）：把一行安裝指令改成自帶兩個旗標，並主推 `install.sh`。**

```sh
npm install -g \
  --allow-remote=all \
  --allow-scripts=https://github.com/gn00678465/StatusLine/releases/latest/download/cc-statusline-npm.tgz \
  https://github.com/gn00678465/StatusLine/releases/latest/download/cc-statusline-npm.tgz
```

已實測此指令在 **npm 10.9.9 / 11.19.0 / 12.0.2 三個大版本上都乾淨成功、零警告零錯誤**（舊版 npm 會靜默忽略它不認識的旗標，見 §3.4）。這是唯一能讓「單行 npm 指令」在 npm 12 存活的寫法。

缺點是 URL 要寫兩次、指令變得很醜，而且要求使用者對一個他還沒讀過的 script 預先授權——安全體感不好。因此這只應該當**過渡方案**，同時把 `install.sh` 提升為 README 的第一選項。

**第二優先（真正的解法，建議排入 v2.1 / v3）：發佈到 npm registry，並改用 `optionalDependencies` 分發平台 binary，主套件完全不要 install script。**

這是 esbuild、sharp、Biome、Playwright 全都走過的路（§4）。它一次解掉上面兩道閘門：

- 從 registry 安裝 → `allow-remote` 完全不適用，第一道閘門消失。
- 主套件沒有任何 lifecycle script → `allowScripts` 沒有東西要審，第二道閘門也消失。

實測 npm 12.0.2 全域安裝 esbuild：postinstall 被封鎖並印出警告，但因為 binary 是靠 `optionalDependencies` 的 `@esbuild/darwin-arm64` 帶進來的，`esbuild --version` 仍正常輸出 `0.28.2`。這證明此模式在 npm 12 下**不需要使用者做任何授權動作**。

本專案要落地需要處理一個差異：cc-statusline 的 binary 目前必須落在 `~/.claude/cc-statusline/`，而 `optionalDependencies` 會把它放進 node_modules。兩個可行做法見 §5.5。

**第三優先：README 補說明。** 必要但不充分——單靠文件無法讓 npm 12 的安裝成功，且 §1.1 結論 B 說明「照 npm 提示做」是錯的，所以文件**必須明確寫出完整 URL 形式**，不能只是轉述 npm 的警告。

**不建議：只改成 explicit install command（`cc-statusline-install`）而繼續走 tarball URL。** 這只解決第二道閘門，npm 12 的 `allow-remote=none` 仍會讓安裝在下載階段就失敗（§5.4）。

### 1.3 其他套件管理器（詳見 §6）

同一波供應鏈防禦已經掃過整個生態，不只 npm。四個工具的一句話結論：

| 工具 | 現況 | 一句話 |
| --- | --- | --- |
| **pnpm 11** | 會壞 | **本次調查中最危險的一個**——`pnpm add -g <URL>` 在非 TTY 下 exit 0、零輸出、binary 沒裝，使用者拿不到任何線索；修法是 `--allow-build='@gn00678465/cc-statusline@<完整 URL>'` |
| **pnpm 10** | 會壞 | 會印出黃框警告並直接告訴你正確的 key，修法是 `--allow-build=@gn00678465/cc-statusline`——**與 pnpm 11 的寫法互不相容**，兩個大版本沒有共通的窄範圍寫法 |
| **bun 1.4** | 會壞 | 訊息最清楚（`Blocked 1 postinstall`）、修法最短（`bun add -g --trust <URL>`，不必重複 URL），因為 bun 用裸套件名比對 |
| **yarn classic 1.22** | **不會壞** | 沒有任何閘門，`yarn global add <URL>` 直接成功 |
| **yarn berry ≥ 4.14** | 不適用 | `enableScripts` 預設已於 4.14.0 翻成 `false`，但 berry 根本沒有 `yarn global add`，不是本專案的安裝路徑 |

兩點值得併入上面的建議排序：

1. **§1.1 結論 B 的「名字比不中」現象不是 npm 獨有。** pnpm 11 為了防同一種 manifest confusion，也在 v11 加上 `trustPackageIdentity` 閘門，使得 tarball URL 安裝**只能**用 `<name>@<完整 URL>` 當 key；registry 套件則裸名字就能中（§6.2.3）。bun 與 yarn 則仍是裸名字比對——UX 較好，但 bun 已為此吃過 [CVE-2026-24910](https://github.com/advisories/GHSA-xp39-vp6q-phvj)。
2. **§1.2 第二優先（registry + optionalDependencies）的價值比原本評估的更高。** 實測顯示該模式在 pnpm 11、yarn 4.18、npm 12 上都是「build script 被擋、但 binary 照樣可用」，等於**一次讓四個套件管理器都回到零授權的單行安裝**（§6.6.2）。

至於 README 要不要為這些工具加說明：**值得，但只加最小的一塊**——一個小表格加一句「若 `~/.claude/cc-statusline/` 是空的就是 script 被擋了」。理由與反對理由見 §6.6.3。

---

## 2. 機制

### 2.1 版本時間軸

| 版本 | 日期 | 變更 | 來源 |
| --- | --- | --- | --- |
| npm 11.9.0 | 2026-02-04 | `config: add --allow-git` | [v11 changelog](https://docs.npmjs.com/cli/v11/using-npm/changelog/) |
| npm 11.15.0 | 2026-05-20 | 加入 `allow-git` / `allow-file` / `allow-directory` / `allow-remote` configs | [v11 changelog](https://docs.npmjs.com/cli/v11/using-npm/changelog/) |
| npm 11.16.0 | 2026-05-27 | **Phase 1 of `allowScripts` opt-in install-script policy**（警告階段） | [v11 changelog](https://docs.npmjs.com/cli/v11/using-npm/changelog/)、[npm/cli#9360](https://github.com/npm/cli/pull/9360)、[npm/cli#9415](https://github.com/npm/cli/pull/9415) |
| npm 12.0.0-pre.1 | 2026-06-19 | default-deny install scripts；`allow-git` 與 `allow-remote` 預設改 `none`；對未知 config/flag 改為報錯 | [v12 changelog](https://docs.npmjs.com/cli/v12/using-npm/changelog/) |
| npm 12.0.0-pre.2 | 2026-06-29 | 核可指令改名，收斂到 `npm install-scripts` 命名空間 | [v12 changelog](https://docs.npmjs.com/cli/v12/using-npm/changelog/) |
| **npm 12.0.0** | **2026-07-08** | **正式版：依賴 lifecycle script 預設封鎖；`allow-git`/`allow-remote` 預設 `none`** | [v12 changelog](https://docs.npmjs.com/cli/v12/using-npm/changelog/) |

規範來源是 [RFC npm/rfcs#868](https://github.com/npm/rfcs/pull/868)，已接受為 [RFC 0054 `make-scripts-install-opt-in`](https://github.com/npm/rfcs/blob/main/accepted/0054-make-scripts-install-opt-in.md)。官方公告為 [GitHub community discussion #198547](https://github.com/orgs/community/discussions/198547)。

### 2.2 「警告但執行」還是「封鎖」

**npm 11.16.0 – 11.x：警告但仍執行。** 這可以從安裝在本機的 npm 11.19.0 原始碼確認。`@npmcli/arborist/lib/arborist/rebuild.js` 的閘門只在**明確 deny** 時才跳過 script：

```js
// node_modules/@npmcli/arborist/lib/arborist/rebuild.js:208
isScriptAllowed(node, this.options.allowScripts) === false
```

`isScriptAllowed` 的回傳是三態：`true`（允許）、`false`（明確拒絕）、`null`（未審核）。因為條件寫的是 `=== false`，未審核（`null`）的套件**照常執行 script**，只是事後被 `lib/utils/reify-output.js` 收集起來印警告。這與使用者觀察到的「出現警告但 script 仍有執行」完全吻合。

**npm 12.0.0 起：封鎖。** 實測 npm 12.0.2 的警告文字本身就變了時態：

- npm 11.19.0：`1 package has install scripts not yet covered by allowScripts:`
- npm 12.0.2：`1 package had install scripts blocked because they are not covered by allowScripts:`

且 marker 檔未被建立，證實 script 確實沒有執行。**注意這是「安靜地跳過並繼續」，安裝本身回傳成功**——對本專案而言這比報錯更危險：使用者會得到一個看似安裝成功、實際上 `~/.claude/cc-statusline/` 裡什麼都沒有的結果。

npm 11 可以用 `--strict-allow-scripts` 提前體驗 npm 12 的嚴格行為，但兩者不完全等價：strict 模式是**硬錯誤**（`ESTRICTALLOWSCRIPTS`，安裝失敗），npm 12 預設是**靜默跳過**（安裝成功但 script 沒跑）。

### 2.3 global install 與 local install 的差異

差異在「政策寫在哪裡」，不在「是否套用」：

- **local（專案內）**：政策寫在專案 `package.json` 的 `allowScripts` 欄位，用 `npm install-scripts approve <pkg>` 維護。RFC 明訂 `--allow-scripts` 旗標**不可**用於專案安裝：「Passing it during a project-scoped `npm install`, `ci`, `update`, or `rebuild` is an error: team-wide policy belongs in `package.json#allowScripts` or `.npmrc`, not in a command-line flag.」
- **global（`-g`）與 `npm exec` / `npx`**：沒有專案 `package.json` 可寫，所以 `npm install-scripts approve -g` 會直接丟 `EGLOBAL` 錯誤。此情境**只能**用 `--allow-scripts` 旗標或 `npm config set allow-scripts ... --location=user`。

本機 npm 11.19.0 的 `npm help install-scripts` 原文：

> This command only works inside a project that has a `package.json`. Running it with `--global` (`-g`) fails with an `EGLOBAL` error, since global installs (`npm install -g`) and one-off executions (`npm exec` / `npx`) have no project `package.json` to write to. To allow install scripts in those contexts, use the `--allow-scripts` flag at install time (for example `npm install -g --allow-scripts=canvas,sharp`) or persist the setting with `npm config set allow-scripts=canvas,sharp --location=user`.

相關 issue：[npm/cli#9457](https://github.com/npm/cli/issues/9457)、[npm/cli#9463](https://github.com/npm/cli/issues/9463)（早期版本在 global 情境誤導使用者去跑會 `EGLOBAL` 的 `npm approve-scripts`，後來已改成現在的 `--allow-scripts` 提示）。

### 2.4 套件名比對細節（研究問題 5 的答案）

**答案：從 tarball URL 安裝時，比對的不是 `package.json` 的 `name`，而是完整的 resolved URL 字串。警告訊息裡顯示的名字只是給人看的 display name，不能拿來當設定值。**

RFC 0054 的規定：

> For file, tarball, and remote URL deps, the `resolved` value is matched as an exact string.

以及一條明確的安全禁令：

> Implementations MUST NOT match keys against: `node.name` (the install location / folder name), `node.package.name` (the tarball's self-reported `package.json#name`), `node.package.version` (the tarball's self-reported `package.json#version`), or `node.package.repository`.

理由是防止 manifest confusion：tarball 可以在自己的 `package.json` 裡宣稱任何名字，所以「套件自己說的名字」不可信，只有 resolver 寫進 lockfile 的 `resolved` 才可信。

實作對應在 `@npmcli/arborist/lib/script-allowed.js`：

```js
const matchRemote = (node, parsed) => {
  return resolvedSourceSpecs(node)
    .some(resolved => resolved === parsed.fetchSpec || resolved === parsed.saveSpec)
}
```

而註冊表套件走的是另一條路 `matchRegistry`，開頭第一件事就是拒絕非 registry 節點：

```js
const matchRegistry = (node, parsed, failClosed) => {
  // If this node is not a registry dep, refuse the match. A registry-style
  // key (`pkg`, `pkg@1`, `pkg@1 || 2`) must not match a tarball or git node
  // even if their names happen to coincide.
  if (!isRegistryNode(node)) {
    return false
  }
  ...
```

**這就是 npm 提示訊息錯誤的根因。** 訊息產生端 `lib/utils/reify-output.js` 用的是 `trustedDisplay(node)`，而該函式在 registry identity 取不到時會退回 `node.name`——也就是 tarball 自報的 `@gn00678465/cc-statusline`：

```js
// lib/utils/reify-output.js
const { name, version } = trustedDisplay(node)
const display = name || '<unknown>'
names.push(display)
...
`Run \`npm install -g --allow-scripts=${list}\` to allow these scripts once, ...`
```

`script-allowed.js` 裡 `trustedDisplay` 自己的註解甚至明講「Do not use for policy matching」，但 remediation 訊息還是拿它去組建議指令。結果就是 npm 對 tarball URL 安裝**產生了一個保證無效的建議**。

實測驗證（npm 11.19.0 + `--strict-allow-scripts`，以及 npm 12.0.2，結果一致）：

| # | `--allow-scripts` 值 | 是否 match |
| --- | --- | --- |
| B | `@testscope/dummy-installer` | 否（strict 下 `ESTRICTALLOWSCRIPTS` 失敗） |
| C | 完整 tarball URL | 是 |
| K | 302 轉址後的最終 URL | 否 |
| J | 使用者輸入的原始（轉址前）URL | 是 |

作為對照，**registry 套件用裸名字是有效的**：npm 12.0.2 執行 `npm install -g --allow-scripts=esbuild esbuild` 完全沒有警告。這正是「發佈到 registry」能大幅改善 UX 的原因。

---

## 3. 設定語法

### 3.1 `allow-scripts`（旗標與 config）

依據 [npm v12 config 文件](https://docs.npmjs.com/cli/v12/using-npm/config/)：

- 預設值：`""`；型別：String（可重複指定）
- 定義：允許執行 install 期 lifecycle script（`preinstall`、`install`、`postinstall`，以及非 registry 來源的 `prepare`）的套件清單，逗號分隔
- 適用情境：`npm exec`、`npx`、`npm install -g`。專案內安裝請改用 `package.json` 的 `allowScripts`

值的解析規則見 `@npmcli/config/lib/parse-allow-scripts-list.js`：接受字串或字串陣列，**以逗號切分**後 trim，空項目略過。因為以逗號切分，值本身不能含逗號（一般 URL 不含逗號，本專案不受影響）。

寫法對照：

```sh
# 一次性（registry 套件，用名字）
npm install -g --allow-scripts=canvas,sharp <pkg>

# 一次性（tarball URL 安裝，必須用完整 URL）
npm install -g --allow-scripts=https://example.com/pkg.tgz https://example.com/pkg.tgz

# 持久化到 user .npmrc
npm config set allow-scripts=canvas,sharp --location=user
```

`package.json` 的 `allowScripts` 欄位支援三態與更精細的 key（RFC 0054）：

- registry 裸名：`"sharp": true`（任何版本）
- registry 釘版：`"sharp@0.34.0": true`；多版本 `"sharp@0.33.2 || 0.33.3": true`
- 明確拒絕：`false`，且一律 name-only（asymmetric-pin rule），deny 優先於 allow
- git：以正規化 SSH URL + short-SHA 前綴比對
- file / directory / tarball / remote：`resolved` 字串精確比對

維護指令（npm 12 命名空間）：

```
npm install-scripts approve <pkg> [<pkg> ...]
npm install-scripts approve --all
npm install-scripts deny <pkg> [<pkg> ...]
npm install-scripts ls
npm install-scripts prune
```

### 3.2 相關的其他 config

| Config | 預設（npm 12） | 型別 | 說明 |
| --- | --- | --- | --- |
| `allow-remote` | `none` | `all` / `none` / `root` | 限制從 URL 抓取依賴。`root` 只允許專案 `package.json` 裡定義的 URL；實測 CLI 直接給 URL 做 global 安裝時 `root` 亦可通過。指向設定的 registry 主機的 tarball 不受影響 |
| `allow-git` | `none` | `all` / `none` / `root` | 同上，針對 git 依賴 |
| `strict-allow-scripts` | `false` | Boolean | 把警告升級為硬錯誤（`ESTRICTALLOWSCRIPTS`）。可在 npm 11 上預演 npm 12 的嚴格度 |
| `dangerously-allow-all-scripts` | `false` | Boolean | 完全繞過 `allowScripts` 政策，執行所有 script。官方定位為「migration escape hatch only」 |
| `ignore-scripts` | `false` | Boolean | 不執行任何 script |

### 3.3 `--ignore-scripts` 與 `allow-scripts` 的關係

`--ignore-scripts` **優先權最高**，會蓋過 `strict-allow-scripts` 與 `dangerously-allow-all-scripts`。程式碼中 `collectUnreviewedScripts` 在 `ignoreScripts` 為真時直接回傳空陣列（除非呼叫端指定 `includeWhenIgnored`，供 approve/deny 列表使用）：

```js
if ((ignoreScripts && !includeWhenIgnored) || dangerouslyAllowAllScripts) {
  return []
}
```

實測 npm 11.19.0：`--ignore-scripts` 安裝時完全沒有 install-scripts 警告，marker 未產生。

兩者語意不同：`--ignore-scripts` 是「什麼都別跑」，`allowScripts` 是「只跑我審過的」。本專案 `npm/postinstall.js:213` 已經有針對前者的處理：

```js
if (process.env.npm_config_ignore_scripts === "true") {
    throw new Error("npm was invoked with --ignore-scripts; reinstall without it to run the installer");
}
```

但這段程式碼**對 npm 12 的封鎖無效**——script 根本不會被啟動，所以沒有任何機會印出提示。這是現行實作的一個盲點：npm 12 之下使用者不會看到本專案的任何說明文字，只會看到 npm 的（錯誤的）建議。

### 3.4 舊版 npm 對未知旗標的行為（研究問題 4b 的答案）

**舊版 npm 靜默忽略未知旗標，不會報錯。** 實測 npm 10.9.9：

```
$ npm --allow-scripts=@foo/bar --version
10.9.9        # exit 0
```

實際安裝也一樣正常：npm 10.9.9 帶 `--allow-remote=all --allow-scripts=<URL>` 從 tarball URL 全域安裝，成功且 postinstall 有執行。

需要留意的反向風險：npm 12.0.0-pre.1 起「error on unknown configs, flags, and abbreviations」。也就是說**未來的 npm 對打錯的旗標會直接報錯**。`allow-remote` 與 `allow-scripts` 在 npm 12 都是已知 config，所以本建議的指令安全；但這代表日後不能再假設「多寫個旗標最多被忽略」。

三版本交叉實測（本次調查實際執行，皆為零警告零錯誤且 postinstall 有執行）：

| npm | 結果 |
| --- | --- |
| 10.9.9 | RAN，0 warn/error |
| 11.19.0 | RAN，0 warn/error |
| 12.0.2 | RAN，0 warn/error |

---

## 4. 生態對策

查閱各套件**已發佈的 registry manifest**（一手資料，用 `npm view <pkg>@latest scripts optionalDependencies` 取得，2026-08-23）：

| 套件 | 版本 | install script | optionalDependencies | 策略 |
| --- | --- | --- | --- | --- |
| `@biomejs/biome` | 2.5.10 | **無** | 8 個 `@biomejs/cli-*` | 純 optionalDependencies |
| `sharp` | 0.35.3 | **無 postinstall** | 25 個 `@img/sharp-*` | 純 optionalDependencies |
| `playwright` | 1.62.1 | **無** | 1（`fsevents`） | 瀏覽器改由顯式 `playwright install` 下載 |
| `@playwright/test` | 1.62.1 | **無** | 0 | 同上 |
| `esbuild` | 0.28.2 | `postinstall: node install.js` | 26 個 `@esbuild/*` | **混合模式**：postinstall 保留但非必要 |

### 4.1 esbuild 仍是混合模式，但 postinstall 已是「可有可無」

esbuild 目前（0.28.2）**確實仍保留 postinstall**，同時列出 26 個平台 optionalDependencies。關鍵在於 postinstall 只是最佳化／校驗，binary 本身由 optionalDependencies 帶進來。

本次實測（npm 12.0.2，全域安裝，未給任何旗標）：

```
added 2 packages in 413ms
npm warn install-scripts 1 package had install scripts blocked because they are not covered by allowScripts:
npm warn install-scripts   esbuild@0.28.2 (postinstall: node install.js)
```

postinstall 被封鎖，但接著執行 binary：

```
$ <prefix>/bin/esbuild --version
0.28.2
```

**完全正常。** `added 2 packages` 即 `esbuild` + `@esbuild/darwin-arm64`。這是「optionalDependencies 模式在 npm 12 下免授權可用」最直接的證據。

esbuild 改用 optionalDependencies 的原始 PR 是 [evanw/esbuild#1621](https://github.com/evanw/esbuild/pull/1621)，動機明確包含「post-install scripts are disabled」的情境。後續關於是否徹底移除 postinstall 的討論見 [#4085](https://github.com/evanw/esbuild/issues/4085) 與 [#4475](https://github.com/evanw/esbuild/issues/4475)（截至查閱時 #4475 尚無維護者結論，故此處不對 esbuild 的未來走向下斷言）。

### 4.2 npm 官方給套件作者的建議

[官方公告](https://github.com/orgs/community/discussions/198547) 對維護者的指引是兩條並行：

1. 在 README 記錄需求，並連結到核可指令的說明；
2. **降低對 script 的需求**——改用 `prebuild` / `prebuildify` / `node-pre-gyp` 出貨預編譯 binary，或把設定動作移到使用者顯式呼叫的 post-install 指令。

Playwright 正是第二條路的代表：套件本身零 script，瀏覽器由使用者顯式執行 `playwright install` 下載。

### 4.3 對本專案的啟示

生態的共識非常一致：**不要依賴 install script 來取得 binary。** 五個受查套件中有四個已經完全沒有 install script，唯一保留的 esbuild 也已經讓 postinstall 變成非必要路徑。cc-statusline 目前是「postinstall 是唯一取得 binary 的途徑」，屬於這波變更中受衝擊最大的形態。

---

## 5. 本專案修正選項評估

先重述本專案的兩個硬約束：

1. binary 最終必須可以被 Claude Code 的 `statusLine.command` 指到（現行 README 寫死 `~/.claude/cc-statusline/cc-statusline`）；
2. 安裝來源目前是 GitHub Release 的 tarball URL，**不是 npm registry**——這是 `allow-remote` 閘門的成因。

### 5.1 選項 a：README 補充說明

改動最小，但**不充分**，而且如果只是照抄 npm 的警告文字會**寫出錯的指令**（§2.4）。文件裡必須明確給出完整 URL 形式。

- 可行性：高
- npm 11 UX：使用者仍會先看到警告，才回頭翻文件
- npm 12 UX：**無效**，`allow-remote=none` 讓安裝在下載階段就失敗，文件救不了
- 評價：必要但不能單獨採用

### 5.2 選項 b：一行安裝指令內建旗標

```sh
npm install -g \
  --allow-remote=all \
  --allow-scripts=https://github.com/gn00678465/StatusLine/releases/latest/download/cc-statusline-npm.tgz \
  https://github.com/gn00678465/StatusLine/releases/latest/download/cc-statusline-npm.tgz
```

- 可行性：高，且已跨 npm 10/11/12 實測通過（§3.4）
- 向後相容：安全。舊版 npm 靜默忽略未知旗標
- 版本穩定性：`latest/download` URL 不含版號，且 npm 記錄的是轉址前的原始 URL（§1.1 結論 C），因此 allow key 不需隨版本更新
- 缺點：
  - URL 必須重複兩次，指令冗長、容易複製錯
  - 要求使用者在尚未檢視 script 的情況下預先授權，安全體感差，也與 npm 這次變更的初衷相左
  - `--allow-remote=all` 是全域放行遠端 tarball，比實際需要的範圍更寬
- 評價：**目前唯一能保住單行 npm 指令的做法，適合當過渡方案**

### 5.3 選項 c：改為 explicit install command

套件提供 `bin`（例如 `cc-statusline-install`），使用者安裝後顯式執行一次下載安裝。

- 解決第二道閘門：是。顯式執行的指令不受 `allowScripts` 管轄
- 解決第一道閘門：**否**。只要安裝來源還是 tarball URL，npm 12 就會在 `EALLOWREMOTE` 卡死
- 對 README／`settings.json` 的影響：`settings.json` 路徑可維持 `~/.claude/cc-statusline/cc-statusline` 不變（因為安裝指令仍可把 binary 放到該處），README 需要多一個步驟
- UX：兩步驟安裝，比現在差，但符合 Playwright 的成熟慣例
- 評價：**方向正確，但必須與「發佈到 registry」綁在一起才有意義**

### 5.4 選項 d：主推 `install.sh`，npm 降為次要

- 可行性：高，`install.sh` 已存在且已驗證 SHA-256
- 完全不受 npm 政策影響
- 缺點：Windows 使用者沒有 `curl | sh` 路徑；且 `curl | sh` 本身有其安全爭議
- 評價：**應立即執行的低成本改善**，但不能取代 npm 路徑（Windows 與習慣 npm 的使用者仍需要它）

### 5.5 選項 e（本調查追加）：發佈到 npm registry + optionalDependencies

這是 §4 生態共識的做法，也是唯一同時解掉兩道閘門的選項。

結構：

- 主套件 `@gn00678465/cc-statusline` 發佈到 npm registry，**不含任何 lifecycle script**
- 六個平台子套件（`@gn00678465/cc-statusline-darwin-arm64` 等），各自帶 `os` / `cpu` 欄位與預編譯 binary
- 主套件用 `optionalDependencies` 列出全部六個，npm 只會安裝符合當前平台的那個

效果：

- `allow-remote`：不適用（registry 來源），第一道閘門消失
- `allowScripts`：沒有 script 要審，第二道閘門消失
- 安裝指令回到最乾淨的 `npm install -g @gn00678465/cc-statusline`
- 已由 esbuild 實測驗證此模式在 npm 12 下免授權可用（§4.1）

需要處理的差異——binary 會落在 node_modules 而非 `~/.claude/cc-statusline/`。兩個做法：

1. **主套件宣告 `bin: { "cc-statusline": ... }`**，讓 npm 把它 link 到全域 bin 目錄，`settings.json` 直接寫 `cc-statusline`。最乾淨，但有風險：Claude Code 執行 `statusLine.command` 時的 `PATH` 未必包含 npm 全域 bin 目錄（尤其本使用者的 node 由 mise 管理，路徑在 `~/.local/share/mise/installs/node/26/bin`）。此假設需要實測確認再採用。
2. **搭配選項 c 的顯式指令**：主套件提供 `cc-statusline install`，執行時把已經躺在 node_modules 裡的平台 binary 複製到 `~/.claude/cc-statusline/`。此時**不需要任何網路下載，也不需要 SHA-256 驗證**（binary 已由 npm 的 registry 完整性機制保證），`npm/postinstall.js` 現有的下載／解壓／驗章邏輯可以整個刪除。`settings.json` 路徑維持不變，README 變成兩步驟。

成本：release workflow 需要新增「打包並發佈七個 npm 套件」的步驟，並設定 npm publish token。現行 `.github/workflows/release.yml` 已經在六個 target 上矩陣建置產出對應 binary，要接上平台子套件的打包相對直接。

評價：**工程成本最高，但唯一能讓安裝 UX 回到單行且長期免維護的選項。建議排入 v2.1 或 v3。**

### 5.6 建議排序

| 排序 | 選項 | 時程 | 理由 |
| --- | --- | --- | --- |
| 1 | d + b + a 併行 | 立即（v2.0.x） | `install.sh` 升為首選；npm 指令補上兩旗標以在 npm 12 存活；README 明確寫出**完整 URL** 形式並說明 npm 自身提示是錯的 |
| 2 | e（registry + optionalDependencies），搭配 c 的顯式 `install` 子命令 | v2.1 / v3 | 一次解掉兩道閘門，安裝回到單行且零授權；與 Biome／sharp／Playwright 一致 |
| 3 | c 單獨採用 | 不建議 | 無法通過 npm 12 的 `allow-remote` 閘門 |

另外兩個立即可做的小修正：

- `npm/postinstall.js:213` 目前只偵測 `--ignore-scripts`。npm 12 的封鎖不會啟動 script，所以該提示永遠不會被看到。建議在 README 明講「若安裝後 `~/.claude/cc-statusline/` 為空，代表 script 被 npm 12 封鎖」，並附上還原指令。
- 若採用選項 b，`README.md` 的「Development and tests」段落與 `tests/test-install.sh` 可考慮加一個帶旗標的 npm 安裝煙霧測試，避免日後 npm 再改預設值時無聲失效。

---

## 6. 其他套件管理器（pnpm / bun / yarn）

調查日期：2026-08-23
實測工具版本：pnpm 10.32.0、pnpm 11.22.0、bun 1.4.0、yarn classic 1.22.22、yarn berry 4.13.0 / 4.14.0 / 4.18.0（皆由 mise 安裝）
實測方法見 §6.7。

### 6.1 一句話結論

| 工具 | 現況會不會壞 | 怎麼修 |
| --- | --- | --- |
| **pnpm 10** | **會壞**，但有清楚警告 | `pnpm add -g --allow-build=@gn00678465/cc-statusline <URL>` |
| **pnpm 11** | **會壞，而且完全無聲**——exit 0、零輸出、binary 沒裝 | `pnpm add -g --allow-build='@gn00678465/cc-statusline@<URL>' <URL>`（key 必須含完整 URL） |
| **bun 1.4** | 會壞，但訊息最清楚、修法最短 | `bun add -g --trust <URL>` |
| **yarn classic 1.22** | **不會壞**，完全沒有閘門 | 不需要處理 |
| **yarn berry ≥ 4.14** | 不適用——berry 沒有 `yarn global add` | 不需要處理 |

**本節最重要的發現：pnpm 11 是所有受查工具中最危險的一個。** npm 12 至少會印警告（§2.2），pnpm 10 會印一個指名道姓的黃框警告，bun 會印 `Blocked 1 postinstall`。只有 pnpm 11 在非 TTY 環境下**什麼都不說**就跳過 postinstall 並回報成功（§6.2.4）。

**第二個發現：pnpm 的 allow key 在 10 與 11 之間完全相反**，兩邊沒有任何共通的窄範圍寫法（§6.2.3）。

### 6.2 pnpm

#### 6.2.1 版本時間軸

| 版本 | 日期 | 變更 | 來源 |
| --- | --- | --- | --- |
| pnpm 10.0.0 | 2025-01-07 | **依賴 lifecycle script 預設不執行**；以 `pnpm.onlyBuiltDependencies` 放行 | [pnpm discussion #8945](https://github.com/orgs/pnpm/discussions/8945)、[v10.0.0 release](https://github.com/pnpm/pnpm/releases/tag/v10.0.0) |
| pnpm 10.3.0 | 2025-02 | 加入 `strictDepBuilds` | [Build Settings](https://pnpm.io/settings/build) |
| pnpm 10.9.0 | 2025-04-21 | 加入 `dangerouslyAllowAllBuilds` | [Build Settings](https://pnpm.io/settings/build) |
| pnpm 10.26.0 | 2025-12-15 | 加入 `allowBuilds` map | [Build Settings](https://pnpm.io/settings/build) |
| **pnpm 11.0.0** | **2026-04-28** | `allowBuilds` 取代並**移除** `onlyBuiltDependencies` / `onlyBuiltDependenciesFile` / `neverBuiltDependencies` / `ignoredBuiltDependencies` / `ignoreDepScripts`；全域安裝改為 isolated global packages；`strictDepBuilds` 預設 `true`；`pnpm approve-builds -g` 不再支援 | [pnpm 11.0 blog](https://pnpm.io/blog/releases/11.0)、[pnpm discussion #11377](https://github.com/orgs/pnpm/discussions/11377)、[v11.0.0 release](https://github.com/pnpm/pnpm/releases/tag/v11.0.0) |

pnpm 10 的原始公告用詞：

> Lifecycle scripts of dependencies are not executed during installation by default!

#### 6.2.2 沒有 `allow-remote` 等價閘門（研究問題 2 的答案）

**pnpm 沒有 npm 12 那道「遠端來源」閘門。** `pnpm add -g <tarball URL>` 在 pnpm 10.32.0 與 11.22.0 上都能直接下載、解壓、安裝成功，不需要任何旗標。本專案只會撞到**第二道**閘門（build script），撞不到第一道。

實測 pnpm 11.22.0 產生的 isolated global lockfile 證實 URL 被原樣接受：

```yaml
'@gn00678465/cc-statusline@https://github.com/gn00678465/StatusLine/releases/latest/download/cc-statusline-npm.tgz':
  resolution: {integrity: sha512-SBDZIiplF9WWX6q2w2060MHAJKODJF/..., tarball: https://github.com/gn00678465/StatusLine/releases/latest/download/cc-statusline-npm.tgz}
```

注意 `tarball:` 記的是**使用者輸入的 `latest/download` URL，不是 302 轉址後的最終 URL**——與 npm 的 §1.1 結論 C 一致。因此 pnpm 的 allow key 同樣**跨版本穩定**，不必每次發版就改。

#### 6.2.3 「全域頂層套件」不享有豁免，且 allow key 在 10 與 11 完全相反

pnpm 11 的 isolated global install 會替每次 `pnpm add -g` 建一個獨立的專案目錄（`{pnpmHomeDir}/global/v11/{hash}/`，內含自己的 `package.json`、`node_modules`、lockfile）。**使用者在指令列指名的那個套件，在這個合成專案裡就是一個 dependency**，所以照樣受 build 閘門管轄，沒有「頂層套件」豁免。這點與 npm 相同。

放行語法的比對規則實測（每列都是乾淨沙箱重跑一次）：

| `--allow-build` 的值 | pnpm 10.32.0 | pnpm 11.22.0 |
| --- | --- | --- |
| `@gn00678465/cc-statusline`（裸套件名） | **有效**，postinstall 執行 | **無效**，靜默跳過 |
| `@gn00678465/cc-statusline@https://.../cc-statusline-npm.tgz`（name@URL） | **硬錯誤** `ERR_PNPM_INVALID_VERSION_UNION` | **有效**，postinstall 執行 |
| `https://.../cc-statusline-npm.tgz`（純 URL） | 未測 | 無效 |
| `cc-statusline`（去 scope） | 未測 | 無效 |
| `*` | 未測 | 無效 |
| `--dangerously-allow-all-builds` | **有效** | **有效** |

**兩個大版本沒有任何共通的窄範圍寫法**：pnpm 10 唯一能用的值在 pnpm 11 無效，pnpm 11 唯一能用的值在 pnpm 10 會直接讓指令失敗。唯一跨版本通用的是 `--dangerously-allow-all-builds`，但它字面上就是「危險地放行全部」，寫進 README 的觀感很差。

原因在 pnpm 11 新增的 `trustPackageIdentity` 閘門。`dist/pnpm.mjs` 內 `createAllowBuildFunction` 的比對器：

```js
const { name, version: version2, nonSemverVersion } = parse5(depPath);
...
if (allowedDepPathBuilds.has(pkgIdWithPatchHash)) {
  return true;
}
...
const trustPackageIdentity = context?.trustPackageIdentity ?? (name != null && version2 != null && nonSemverVersion == null);
if (!trustPackageIdentity)
  return void 0;
if (name != null && expandedAllowed.has(name) || nameAtVersion2 != null && expandedAllowed.has(nameAtVersion2)) {
  return true;
}
```

從 tarball URL 安裝的套件其 `nonSemverVersion` 不為 null（version 位置放的是 URL），於是 `trustPackageIdentity` 為 false，**裸名字的允許清單根本不會被查詢**，只剩下 exact depPath 那條路。這與 npm RFC 0054「MUST NOT match against `node.package.name`」（§2.4）是同一個防 manifest confusion 的設計，pnpm 只是晚一個大版本才補上。

canonical key 的產生規則同樣寫在 bundle 裡：

```js
function allowBuildKeyFromIgnoredBuild(depPath) {
  const pkgIdWithPatchHash = getPkgIdWithPatchHash(depPath);
  const parsed = parse5(pkgIdWithPatchHash);
  if (parsed.nonSemverVersion != null || parsed.name == null)
    return pkgIdWithPatchHash;   // tarball / git → 完整 name@URL
  return parsed.name;            // registry → 裸名字
}
```

**對照組（證明這是 remote-vs-registry 的差別，不是 pnpm 11 壞掉）**：實測 pnpm 11.22.0 執行 `pnpm add -g --allow-build=esbuild esbuild`，裸名字**有效**，postinstall 正常執行。與 npm §2.4 的對照組結論完全一致。

#### 6.2.4 pnpm 11 在非 TTY 下靜默失敗（本節最嚴重的問題）

實測 pnpm 11.22.0，`pnpm add -g <URL>`，stdin 導向 `/dev/null`：

```
-- exit=0
-- STDOUT:
Packages: +1
+
global:
+ @gn00678465/cc-statusline 2.0.0
Done in 1.7s using pnpm v11.22.0
-- STDERR:
（空）
-- marker: skipped
```

**exit code 0、stderr 全空、stdout 完全沒有提到 build script，而 `~/.claude/cc-statusline/` 是空的。** 使用者拿不到任何線索。

對比 pnpm 10.32.0 同樣的指令：

```
╭ Warning ─────────────────────────────────────────────────────────────────────╮
│   Ignored build scripts: @gn00678465/cc-statusline@https://github.com/gn00   │
│   678465/StatusLine/releases/latest/download/cc-statusline-npm.tgz.          │
│   Run "pnpm approve-builds -g" to pick which dependencies should be          │
│   allowed to run scripts.                                                    │
╰──────────────────────────────────────────────────────────────────────────────╯
```

pnpm 10 不但警告，還**直接把正確的 key 印出來了**——這比 npm 的錯誤建議（§1.1 結論 B）好得多。

差異的成因有兩層：

1. 警告的印製條件是 `!opts3.pnpmConfig?.strictDepBuilds`，而 `strictDepBuilds` 在 v11 預設為 `true`，所以警告被抑制：

   ```js
   // ../cli/default-reporter/lib/reporterForClient/reportIgnoredBuilds.js
   if (ignoredScripts.packageNames && ignoredScripts.packageNames.length > 0 && !opts3.pnpmConfig?.strictDepBuilds) {
     const msg = boxen(`Ignored build scripts: ...`)
   ```

2. 全域安裝路徑不走 strict 硬錯誤，改走**互動式核可 prompt**（`approve-builds` 的錯誤訊息自己講明：「pnpm will also prompt to allow builds interactively during global install」）。在非 TTY 下這個 prompt 直接落空，於是既沒有警告也沒有錯誤。實測在 PTY 下（`script -q /dev/null pnpm add -g <URL>`）同一道指令會**掛住等待輸入**，反證了 prompt 的存在。

順帶兩個陷阱：

- `--strict-dep-builds` / `--no-strict-dep-builds` **不是 CLI 旗標**。pnpm 11.22.0 回 `[ERROR] Unknown option: 'strict-dep-builds'`，它只能寫在 `pnpm-workspace.yaml`，而全域安裝沒有那個檔。
- pnpm 10 的警告建議跑 `pnpm approve-builds -g`，但升到 pnpm 11 之後這條指令會直接丟錯：

  ```js
  throw new PnpmError("APPROVE_BUILDS_NOT_SUPPORTED_WITH_GLOBAL", '"approve-builds" is not supported with global packages', {
    hint: 'Use --allow-build when installing globally, e.g. "pnpm add -g --allow-build=<pkg> <pkg>". pnpm will also prompt to allow builds interactively during global install.'
  })
  ```

  官方文件亦記載 `--global` 已於 v11.0.0 移除（[approve-builds 文件](https://pnpm.io/cli/approve-builds)）。

### 6.3 bun

實測 bun 1.4.0。

**lifecycle scripts 政策**：預設封鎖依賴的 lifecycle scripts，自 bun 1.0.31（2024-03-15）起提供 `trustedDependencies` 工作流。官方文件用詞：「Bun does not execute arbitrary lifecycle scripts by default, unlike other npm clients.」（[Lifecycle scripts](https://bun.com/docs/install/lifecycle)）。bun 另有一份 curated 預設信任清單，**僅適用於 npm 來源**；`file:` / `link:` / `git:` / `github:` 來源必須顯式列入 `trustedDependencies`。與 pnpm 一樣，全域安裝的頂層套件不享有豁免。

**從遠端 tarball URL 全域安裝**：支援，且沒有任何來源限制。實測 `bun add -g <URL>` 成功。

**現況實測**：

```
$ bun add -g https://github.com/.../cc-statusline-npm.tgz
installed @gn00678465/cc-statusline@https://github.com/.../cc-statusline-npm.tgz
1 package installed [902.00ms]

Blocked 1 postinstall. Run `bun pm -g untrusted` for details.
```

exit 0、binary 沒裝，**但訊息清楚且 remediation 指令正確**——這是所有受查工具裡診斷體驗最好的。`bun pm -g untrusted` 會進一步印出被擋的是哪個 script：

```
./node_modules/@gn00678465/cc-statusline @https://github.com/.../cc-statusline-npm.tgz
 » [postinstall]: node postinstall.js
```

**放行語法**：兩條路都實測有效，postinstall 執行、binary 落地。

```sh
bun add -g --trust https://github.com/gn00678465/StatusLine/releases/latest/download/cc-statusline-npm.tgz
# 或事後補救
bun pm -g trust @gn00678465/cc-statusline
```

`--trust` 的官方定義：「Add to `trustedDependencies` in the project's `package.json` and install the package(s)」（`bun add --help`，bun 1.4.0）。

**套件識別比對規則**：**用裸套件名，即使是從 tarball URL 安裝也一樣。** 這是 bun 與 npm 12 / pnpm 11 最大的差異，也是它 UX 最好的原因——旗標只要一個 `--trust`，不必把 URL 重複一次。實測 `bun pm -g trust @gn00678465/cc-statusline` 對一個 URL 安裝的套件有效，事後寫進全域 `package.json`：

```json
{
  "dependencies": { "@gn00678465/cc-statusline": "https://github.com/.../cc-statusline-npm.tgz" },
  "trustedDependencies": ["@gn00678465/cc-statusline"]
}
```

代價是安全性：名字由 tarball 自報，正是 npm RFC 0054 明令禁止拿來比對的東西。bun 已經因為相關問題吃過一個 CVE——[CVE-2026-24910 / GHSA-xp39-vp6q-phvj](https://github.com/advisories/GHSA-xp39-vp6q-phvj)（medium，2026-01-28）：「In Bun before 1.3.5, the default trusted dependencies list (aka trust allow list) can be spoofed by a non-npm package in the case of a matching name (for file, link, git, or github).」bun 1.3.5（2025-12-17）修的是**預設清單**不再讓非 npm 來源憑名字繼承信任；使用者顯式寫下的 `trustedDependencies` 仍是名字比對（本次在 1.4.0 上實測確認）。

### 6.4 yarn classic（1.x）

實測 yarn 1.22.22（經 corepack）。

**完全沒有閘門，現況不會壞。** `yarn global add <URL>` 直接成功，postinstall 執行，binary 落到 `~/.claude/cc-statusline/`：

```
[4/4] Building fresh packages...
Done in 1.91s.
warning "@gn00678465/cc-statusline@2.0.0" has no binaries
```

（`has no binaries` 只是提醒此套件沒有宣告 `bin`，對本專案無影響——binary 是由 postinstall 放到 `~/.claude/` 的。）

yarn 1.22.22 是 classic 線的最後版本，處於 maintenance mode，不預期會加入類似的 script 閘門。**本專案在 yarn classic 上不需要任何處理。**

### 6.5 yarn berry（2.x–4.x）

實測 yarn 4.13.0 / 4.14.0 / 4.18.0。

**`enableScripts` 的預設值已於 Yarn 4.14.0（2026-04-16）從 `true` 翻成 `false`。** 這個變更此前未見於官方 changelog 頁面，是本次由 PR 與實測交叉確認的：

- [yarnpkg/berry#7089 — Makes `enableScripts: false` the default](https://github.com/yarnpkg/berry/pull/7089)，merged 2026-03-31。作者說明：「I was planning to wait until the next major to land this, but considering the regularity of package compromissions, I think we need to address it sooner than that.」
- 版本落點由實測 bisect 確定（`yarn config get enableScripts`）：

  | yarn 版本 | 發佈日 | `enableScripts` | tarball URL 安裝時 postinstall |
  | --- | --- | --- | --- |
  | 4.13.0 | 2026-03-03 | `true` | 執行 |
  | 4.14.0 | 2026-04-16 | `false` | 跳過（`YN0004`） |
  | 4.18.0 | 2026-07-29 | `false` | 跳過（`YN0004`） |

目前 [yarnrc 文件](https://yarnpkg.com/configuration/yarnrc) 已更新為 default `false`：

> If false (the default), Yarn will not execute the `postinstall` scripts from third-party packages when installing the project (workspaces will still see their postinstall scripts evaluated, as they're assumed to be safe if you're running an install within them).

注意括號裡的例外——**workspace 自身的 script 仍會執行**，這就是 berry 版本的「頂層 vs 依賴」差別政策。但本專案是第三方依賴，不適用。

**berry 沒有全域安裝路徑。** `yarn global add` 在 berry 已移除（實測 4.18.0 會把 `global` 當成套件名去解析而報 Internal Error）。berry 也要求 URL 必須帶套件名前綴：

```
Usage Error: It seems you are trying to add a package using a https:... url; we now require package names to be explicitly specified.
Try running the command again with the package name prefixed: yarn add my-package@https:...
```

**放行語法**（兩者都實測有效，postinstall 執行）：

```yaml
# .yarnrc.yml
enableScripts: true
```

```json
// package.json — 只放行單一套件，比全域開關安全
{
  "dependenciesMeta": { "@gn00678465/cc-statusline": { "built": true } }
}
```

`dependenciesMeta` 的 key 用**裸套件名**即可（與 bun 同、與 npm/pnpm 相反），實測在 `enableScripts` 為預設 `false` 時仍能單點放行該套件。

**對本專案的意義有限**：berry 使用者本來就不會用 `yarn global add` 裝 CLI 工具，該情境在 berry 只剩 `yarn dlx`（一次性執行，不適合 cc-statusline 這種要常駐給 Claude Code 呼叫的 binary）。berry 不是本專案的目標安裝路徑。

### 6.6 對本專案的結論與 README 建議

#### 6.6.1 可用的指令（全部經本次實測）

```sh
# pnpm 11.x
pnpm add -g \
  --allow-build='@gn00678465/cc-statusline@https://github.com/gn00678465/StatusLine/releases/latest/download/cc-statusline-npm.tgz' \
  https://github.com/gn00678465/StatusLine/releases/latest/download/cc-statusline-npm.tgz

# pnpm 10.x（注意：與 pnpm 11 的寫法互不相容）
pnpm add -g --allow-build=@gn00678465/cc-statusline \
  https://github.com/gn00678465/StatusLine/releases/latest/download/cc-statusline-npm.tgz

# bun
bun add -g --trust https://github.com/gn00678465/StatusLine/releases/latest/download/cc-statusline-npm.tgz

# yarn classic — 不需任何旗標
yarn global add https://github.com/gn00678465/StatusLine/releases/latest/download/cc-statusline-npm.tgz
```

#### 6.6.2 §5.5 的 registry + optionalDependencies 方案同樣解掉 pnpm / bun / yarn

這是本節對 §5 建議排序最有力的補強。實測「主套件無 script、binary 靠 optionalDependencies」的 esbuild，在**不給任何旗標**的情況下：

| 工具 | 安裝結果 | binary 可用？ |
| --- | --- | --- |
| pnpm 11.22.0（`add -g esbuild`） | build script 被擋 | **是**，`esbuild --version` → `0.28.2` |
| yarn 4.18.0（專案內 `add esbuild`） | `YN0004` build scripts disabled | **是**，`0.28.2` |
| bun 1.4.0（`add -g esbuild`） | 未被擋（esbuild 在 bun 預設信任清單內，故未直接驗證 fallback 路徑） | 是，`0.28.2` |
| npm 12.0.2 | build script 被擋（§4.1） | **是**，`0.28.2` |

換句話說，§5.5 的方案不只是「解決 npm 12」，而是**一次讓四個套件管理器都回到零授權的單行安裝**。這讓它的優先序比 §5.6 原本評估的更高。

#### 6.6.3 README 值不值得動

**建議：值得，但只加最小的一塊——一個四行的小表格加一句自我診斷提示，不要為每個工具各寫一段。**

支持加的理由只有一個，但它夠強：**pnpm 11 是靜默失敗**。使用者不會看到錯誤、不會看到警告，只會發現 Claude Code 的 statusline 不動，而且沒有任何線索指向「script 被套件管理器擋了」。這是本次調查中**唯一一個文件能實質降低診斷成本**的情境——bun 會印 `Blocked 1 postinstall` 並附上正確指令，pnpm 10 會印出完整的 key，yarn classic 根本不會壞，這三者使用者自己就能查出來。

反對過度投入的理由（採納 §1.2 的排序邏輯）：本專案是單一使用者專案，pnpm/bun/yarn 的優先序低於 npm，而 `install.sh` 對這四個工具**全都是不受影響的逃生路**——它不經過任何套件管理器。

因此建議的改動範圍是：在 README 現有「npm installer」段落之後補一小節，內容不超過表格加兩句話，並明確寫出「若 `~/.claude/cc-statusline/` 是空的，代表 script 被你的套件管理器擋了，改用 `install.sh` 或加上對應旗標」。至於 pnpm 10 與 pnpm 11 寫法不相容這件事，表格裡分兩列列出即可，不需要解釋原因。

同時提醒：§5.6 的「立即可做的小修正」提到 `npm/postinstall.js:213` 的 `--ignore-scripts` 提示在 npm 12 下永遠不會被看到——這個盲點對 pnpm 11 / bun / yarn berry **同樣成立**（script 根本不會被啟動，程式碼沒有機會說話）。這再次說明自我診斷的說明只能寫在 README，寫在 postinstall 裡沒有用。

### 6.7 本節實測方法

所有測試都在 `HOME` 被指向沙箱目錄的環境下執行——`npm/postinstall.js:197` 用 `process.env.HOME` 決定安裝位置，因此覆寫 `HOME` 就能讓 binary 落到沙箱的 `$HOME/.claude/cc-statusline/`，同時把 pnpm（`PNPM_HOME`）、bun（`BUN_INSTALL`）、yarn 的全域目錄與快取一併隔離。每個測試案例前都會 `rm -rf` 重建沙箱，確保沒有殘留狀態或快取影響結果。判定「postinstall 是否執行」的依據是 `$HOME/.claude/cc-statusline/cc-statusline` 是否存在。

實測用的 tarball 是 repo 現成的 release URL `https://github.com/gn00678465/StatusLine/releases/latest/download/cc-statusline-npm.tgz`（302 轉址到 `v2.0.0/cc-statusline-npm.tgz`）。使用者真實的 `~/.claude/cc-statusline/` 全程未被觸碰，測試前後 SHA-256 一致（`db9f7e6b0abf...`）。

pnpm 的內部機制（`createAllowBuildFunction`、`allowBuildKeyFromIgnoredBuild`、`reportIgnoredBuilds`、`approveBuilds`）引用自本機安裝的 `~/.local/share/mise/installs/pnpm/11.22.0/dist/pnpm.mjs`（打包後的單一 bundle，原始路徑保留在註解中）。

---

## 7. 一手來源清單

**npm 官方規範與公告**

- [RFC npm/rfcs#868 — Make install scripts opt-in](https://github.com/npm/rfcs/pull/868)
- [RFC 0054（已接受）— make-scripts-install-opt-in](https://github.com/npm/rfcs/blob/main/accepted/0054-make-scripts-install-opt-in.md)
- [Preparing for npm v12: install scripts and non-registry sources become opt-in — GitHub community discussion #198547](https://github.com/orgs/community/discussions/198547)

**npm 官方文件**

- [npm v12 config 文件（`allow-scripts`、`allow-remote`、`allow-git`、`strict-allow-scripts`、`dangerously-allow-all-scripts`、`ignore-scripts`）](https://docs.npmjs.com/cli/v12/using-npm/config/)
- [npm v12 `npm install-scripts` 指令文件](https://docs.npmjs.com/cli/v12/commands/npm-install-scripts/)
- [npm v11 `npm approve-scripts` 指令文件](https://docs.npmjs.com/cli/v11/commands/npm-approve-scripts/)
- [npm v12 changelog](https://docs.npmjs.com/cli/v12/using-npm/changelog/)
- [npm v11 changelog](https://docs.npmjs.com/cli/v11/using-npm/changelog/)

**npm CLI 實作與 issue**

- [npm/cli#9360 — feat: Phase 1 of `allowScripts` opt-in install-script policy](https://github.com/npm/cli/pull/9360)
- [npm/cli#9415 — feat: Phase 1 of `allowScripts` opt-in install-script policy](https://github.com/npm/cli/pull/9415)
- [npm/cli#9457 — Unreviewed-scripts warning suggests `npm approve-scripts` during global installs, where it can't work](https://github.com/npm/cli/issues/9457)
- [npm/cli#9463 — allow-scripts warning on global install suggests `npm approve-scripts`, which errors EGLOBAL](https://github.com/npm/cli/issues/9463)
- [npm/cli#9488 — `npm i` shows `allow-scripts` uncovered dependency warning even if dependency was already approved](https://github.com/npm/cli/issues/9488)
- [npm/cli#9562 — `npm ci` with `strict-allow-scripts` rejects package that `approve-scripts` cannot see](https://github.com/npm/cli/issues/9562)

**本機 npm 11.19.0 原始碼**（路徑相對於 `~/.local/share/mise/installs/node/26/lib/node_modules/npm/`）

- `node_modules/@npmcli/arborist/lib/script-allowed.js` — 政策比對器（`matchRegistry` / `matchRemote` / `matchGit` / `trustedDisplay`）
- `node_modules/@npmcli/arborist/lib/unreviewed-scripts.js` — 未審核 script 收集與 strict 錯誤
- `node_modules/@npmcli/arborist/lib/arborist/rebuild.js:208` — 執行閘門（`=== false` 才跳過，證明 npm 11 為 advisory）
- `node_modules/@npmcli/config/lib/definitions/definitions.js` — config 定義
- `node_modules/@npmcli/config/lib/parse-allow-scripts-list.js` — 逗號切分解析
- `lib/utils/reify-output.js` — 警告訊息與 remediation 產生（錯誤建議的來源）
- `lib/utils/allow-scripts-remediation.js` — `npm config set allow-scripts` 建議字串
- `lib/utils/check-allow-scripts.js`

**生態套件已發佈 manifest**（`npm view <pkg>@latest`，2026-08-23）

- `esbuild@0.28.2`、`@biomejs/biome@2.5.10`、`sharp@0.35.3`、`playwright@1.62.1`、`@playwright/test@1.62.1`
- [evanw/esbuild#1621 — install using "optionalDependencies"](https://github.com/evanw/esbuild/pull/1621)
- [evanw/esbuild#4085 — Consider removing `postinstall` script](https://github.com/evanw/esbuild/issues/4085)
- [evanw/esbuild#4475 — Reconsidering the removal the postinstall script](https://github.com/evanw/esbuild/issues/4475)

**pnpm 官方文件與公告**（§6）

- [pnpm 10 公告 — discussion #8945](https://github.com/orgs/pnpm/discussions/8945)、[v10.0.0 release](https://github.com/pnpm/pnpm/releases/tag/v10.0.0)（2025-01-07）
- [pnpm 11 公告 — discussion #11377](https://github.com/orgs/pnpm/discussions/11377)、[pnpm 11.0 blog](https://pnpm.io/blog/releases/11.0)、[v11.0.0 release](https://github.com/pnpm/pnpm/releases/tag/v11.0.0)（2026-04-28）
- [pnpm Build Settings（`allowBuilds`、`dangerouslyAllowAllBuilds`、`strictDepBuilds`，含已於 v11 移除的舊設定）](https://pnpm.io/settings/build)
- [pnpm `approve-builds` 指令文件（`--global` 已於 v11.0.0 移除）](https://pnpm.io/cli/approve-builds)
- [pnpm Global Packages（isolated global installs、`--allow-build`）](https://pnpm.io/global-packages)

**bun 官方文件與安全公告**（§6）

- [bun Lifecycle scripts / `trustedDependencies`](https://bun.com/docs/install/lifecycle)
- [bun guide — Add a trusted dependency](https://bun.com/docs/guides/install/trusted)
- [bun `bun add` CLI 文件（tarball URL、`-g`、`--trust`）](https://bun.com/docs/cli/add)
- [GHSA-xp39-vp6q-phvj / CVE-2026-24910 — Bun 預設信任清單可被非 npm 來源以同名冒充（修於 bun 1.3.5）](https://github.com/advisories/GHSA-xp39-vp6q-phvj)

**yarn 官方文件與 PR**（§6）

- [yarnrc 設定文件（`enableScripts`、`supportedArchitectures`、`unsafeHttpWhitelist`）](https://yarnpkg.com/configuration/yarnrc)
- [yarnpkg/berry#7089 — Makes `enableScripts: false` the default](https://github.com/yarnpkg/berry/pull/7089)（merged 2026-03-31，落在 Yarn 4.14.0）

**本機 pnpm 11.22.0 原始碼**（`~/.local/share/mise/installs/pnpm/11.22.0/dist/pnpm.mjs`，單一 bundle，原始路徑保留於註解）

- `../building/policy/lib/...` — `createAllowBuildFunction`（`trustPackageIdentity` 閘門）、`allowBuildKeyFromIgnoredBuild`（canonical key 產生規則）、`addAllowBuildRule` / `isDepPathAllowBuildKey`
- `../cli/default-reporter/lib/reporterForClient/reportIgnoredBuilds.js` — 警告在 `strictDepBuilds` 為真時被抑制
- `../building/commands/lib/policy/approveBuilds.js` — `APPROVE_BUILDS_NOT_SUPPORTED_WITH_GLOBAL`

**本次實測（§1–§5，npm）**

沙箱內以 `npm install -g --prefix <sandbox>` 安裝自建測試套件（`@testscope/dummy-installer`，postinstall 寫 marker 檔），tarball 由本機 HTTP server 提供（含一組 302 轉址端點以模擬 GitHub `releases/latest/download`）。npm 10.9.9 與 npm 12.0.2 以 `npm install npm@10 / npm@12` 安裝於沙箱後直接呼叫其 `.bin/npm`。所有 §1、§2.2、§2.4、§3.3、§3.4、§4.1 的實測結論皆出自這組測試。

**本次實測（§6，pnpm / bun / yarn）**

工具版本：pnpm 10.32.0、pnpm 11.22.0、bun 1.4.0、yarn classic 1.22.22、yarn berry 4.13.0 / 4.14.0 / 4.18.0（皆由 mise 安裝）。方法與隔離手段見 §6.7；使用真實 release URL，使用者的 `~/.claude/cc-statusline/` 全程未被觸碰。
