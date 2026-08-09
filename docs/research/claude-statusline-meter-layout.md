# `nilbuild/claude-statusline` meter 版面研究

日期：2026-08-09

## 結論

`nilbuild/claude-statusline` 的 rate-limit meter **沒有 dot 間隔**。它將
filled `●` 與 empty `○` 直接串接，形成連續十格：

```text
●●○○○○○○○○
```

因此它不是「間隔稍微大一點、但不連在一起」的折衷方案；它也沒有可切換
dot/bar 或 spacing 的設定：這個樣式直接硬編碼在 `build_bar()`。

## 固定版本與來源可信度

本研究只讀取本機 clone 的 Git object，不受該 clone 未提交檔案影響：

| 項目 | 值 |
| --- | --- |
| 本機目錄 | `/d/Projects/status-line-dev/claude-statusline` |
| remote | `https://github.com/nilbuild/claude-statusline.git` |
| 固定 commit | [`ea02c0e6dcd532fea6056f7eec2b7545b3666248`](https://github.com/nilbuild/claude-statusline/tree/ea02c0e6dcd532fea6056f7eec2b7545b3666248) |
| branch / remote-tracking ref | `main` / `origin/main` |
| commit 說明 | `Bump version 1.0.6` |

本機 worktree 的 `README.md`、`bin/statusline.sh` 等檔案當時有未提交修改；本
文件所有程式碼、README 與 demo 判讀都以 `git show ea02c0e:<path>` 讀取的
commit blob 為準，而非 worktree 檔案。

## 實作：連續 dots、固定十格

[`bin/statusline.sh` L35–50](https://github.com/nilbuild/claude-statusline/blob/ea02c0e6dcd532fea6056f7eec2b7545b3666248/bin/statusline.sh#L35-L50)
的 `build_bar(pct, width)` 行為是：

1. 將 percent 限制在 0–100。
2. 依 `pct * width / 100` 算出 filled / empty 數量。
3. 迴圈以 `filled_str+="●"` 與 `empty_str+="○"` 逐顆附加 glyph。
4. 輸出 `${filled_str}${empty_str}`；兩個迴圈和兩段字串之間都沒有空白、thin
   space 或其他 separator。

[`bin/statusline.sh` L290–312](https://github.com/nilbuild/claude-statusline/blob/ea02c0e6dcd532fea6056f7eec2b7545b3666248/bin/statusline.sh#L290-L312)
將 `bar_width` 固定為 `10`，並將此 builder 用於 `current`（5-hour）與
`weekly`（7-day）兩列。filled dots 使用依整體百分比選出的色彩，empty dots
使用同色的 dim 版本；threshold 為 <50 綠、50–69 橘、70–89 黃、≥90 紅，見
[`color_for_pct()` L26–32](https://github.com/nilbuild/claude-statusline/blob/ea02c0e6dcd532fea6056f7eec2b7545b3666248/bin/statusline.sh#L26-L32)。

此版本的 context 區塊只印百分比（`✍️ 25%`），不使用 meter；meter 僅顯示在
rate-limit 的多行區塊，見 [L171–189](https://github.com/nilbuild/claude-statusline/blob/ea02c0e6dcd532fea6056f7eec2b7545b3666248/bin/statusline.sh#L171-L189)
與 [L290–333](https://github.com/nilbuild/claude-statusline/blob/ea02c0e6dcd532fea6056f7eec2b7545b3666248/bin/statusline.sh#L290-L333)。

## 實際 mock JSON 執行結果

以 commit `ea02c0e` 的 `bin/statusline.sh` 執行 mock JSON（5h 20%、7d 50%），
移除 ANSI escape sequences 後結果如下：

```text
Claude │ ✍️ 25% │ claude-statusline (main*) │ ◑ default

current ●●○○○○○○○○  20%
weekly  ●●●●●○○○○○  50%
```

可見 meter 本體沒有 U+0020、U+2009 或其他分隔 code point；空白只出現在 label
與 meter、meter 與百分比之間。這與 source 直接串接的結論一致。

## README 與 demo 的限制

固定 README 僅在 [L3–5](https://github.com/nilbuild/claude-statusline/blob/ea02c0e6dcd532fea6056f7eec2b7545b3666248/README.md#L3-L5)
概述顯示 limits/directory/git，並嵌入
[`demo.png`](https://github.com/nilbuild/claude-statusline/blob/ea02c0e6dcd532fea6056f7eec2b7545b3666248/.github/demo.png)。沒有 meter
style、glyph 或 spacing 的設定說明。

該 screenshot 視覺上是較舊的 11-dot 示意；執行中的版本化 source 自
`bar_width=10` 固定輸出 10 個位置。若要作實作參考，應以可執行 source 的 10
格、無 separator 規則為準，而不是依 screenshot 推測格數或間距。

## 採用含意

若採用 `claude-statusline` 的 meter 規則，應直接串接十個 `●` / `○`；相較
每個位置以 ASCII space 分隔，單一 meter 會少 9 個 terminal columns。filled
`●` / dim empty `○` 與整列依百分比變色的規則亦可直接沿用。
