#!/usr/bin/env bash
# horse-poc.sh — 純 bash 移植 token-horse 逐幀馬動畫 (Proof of Concept)
#
# 來源原理 (token-horse/horse-token-runner.mjs):
#   - 15 幀，每幀 8 列 × 32 個十六進位字元
#   - 每個 hex 字元 v 編碼上下兩個像素: top=v/4, bottom=v%4 (各 0–3 四級灰階)
#   - 半角區塊渲染: ▀(fg=上) ▄(下) █(滿)，24-bit truecolor，run-length 壓縮
#   - 一個 hex cell 直接對應一個輸出字元 (cell row k → 文字行 k)
#
# 用法:
#   ./horse-poc.sh frame [N]        靜態渲染第 N 幀 (預設 0)
#   ./horse-poc.sh all              依序印出全部 15 幀 (驗證資料)
#   ./horse-poc.sh anim [rate] [s]  以模擬 token 速率 rate(tok/s) 跑 s 秒動畫
#   ./horse-poc.sh tick [rate]      statusline 模式: 讀/寫狀態檔推進一幀並輸出
#
set -u

# ── ANSI / 區塊字元 ────────────────────────────────────────────────
RESET=$'\e[0m'
BG_RESET=$'\e[49m'
BLOCK_TOP=$'▀'      # ▀
BLOCK_BOTTOM=$'▄'   # ▄
BLOCK_FULL=$'█'     # █
BLANK_ANCHOR=$'⠀'   # ⠀ braille blank — statusline host 會 strip 開頭空白

# GREEN_SHADES[1..3] 的 fg / bg truecolor escape (對齊原始 #0f5f24 / #24b84a / #59ff75)
declare -a FG BG
FG[1]=$'\e[38;2;15;95;36m';  BG[1]=$'\e[48;2;15;95;36m'
FG[2]=$'\e[38;2;36;184;74m'; BG[2]=$'\e[48;2;36;184;74m'
FG[3]=$'\e[38;2;89;255;117m';BG[3]=$'\e[48;2;89;255;117m'

# ── 幀資料: 120 列 (15 幀 × 8 列)，flat array，frame f row r = ROWS[f*8+r] ──
ROWS=(
'0000000000000000000000000f190000' '0000000000000000000000045ffef330' '0000000001000000000000055fffccc0' '00000001555113ffffffff777fff0000' '0000001540440ffffffffffffffe0000' '00000000000003feecccccccffa00000' '0000000000000fca00000000f0a00000' '0000000000000d0900000000d0900000'
'0000000000000000000000000f190000' '0000000000000000000000045ffef330' '0000000010000000000000055fffccc0' '0000001555113ffffffff777fffc0000' '000001540440ffffffffffffffc00000' '0000000000003ffccccccccffa000000' '00000000000fca800000013e28000000' '00000000001c008100001c0000000000'
'0000000000000000000000000f190000' '0000000000000000000000045ffef330' '0000001100000000000000055fffccc0' '000015555113fffffffff777fffc0000' '00000400440ffffffffffffffc000000' '000000000003ffccccccccff82000000' '0000000000fca80000003c0028000000' '0000000000cd0810004c000400000000'
'0000000000000000000000000f190000' '0000000000000000000000045ffef330' '0000015510000000000000055fffccc0' '000045445513fffffffff777fffc0000' '00000000000ffffffffffffffc000000' '000000000000ffccccccccff82000000' '00000000000fc82000433c1228000000' '000000000000c1081000000000000000'
'0000000000000000000000000f190000' '0000000000000000000000045ffef330' '0000115100000000000000055fffccc0' '000444455113fffffffff777fffc0000' '00000000400ffffffffffffffc000000' '000000000000ffecccccccff8a000000' '00000000000fc8a00004c3c028000000' '000000000000c1881000000400000000'
'0000000000000000000000000f190000' '0000000000000000000000045ffef330' '0000001000000000000000055fffccc0' '000155555113fffffffff777fffc0000' '00000000400ffffffffffffffc000000' '000000000000ffecccccccffa2000000' '00000000000cf8820004c33c0a000000' '0000000000000c108100000040000000'
'0000000000000000000000000f190000' '0000000000000000000000015ffef330' '0000000000000000000000055fffccc0' '000011111113fffffffff777fffc0000' '00044454400ffffffffffffffc000000' '000000000000feecccccccffa2000000' '000000000000cf820000133c08200000' '0000000000000cd09000000000400000'
'0000000000000000000000000f190000' '0000000000000000000000045ffef330' '0000000000000000000000055fffccc0' '0004111551113ffffffff777fffc0000' '000004444400ffffffffffffffc00000' '0000000000002fffcccccccffb300000' '00000000000288cf0000000003c81000' '0000000000081000c100000040000000'
'0000000000000000000000000f190000' '0000000000000000000000015ffef330' '0000000110000000000000055fffccc0' '0000155555113ffffffff777fffc0000' '000000000000ffffffffffffffc00000' '0000000000002ffccccccccffb300000' '00000000000288f30000000000f81000' '00000000000900004000000000400000'
'0000000000000000000000000f190000' '0000000000000000000000045ffef330' '0000000100000000000000055fffccc0' '000015545513fffffffff777fffc0000' '00044000000ffffffffffffffc000000' '000000000002ffcccccccccff3000000' '00000000002bc000000000008ecc1000' '00000000180c10000000000000400000'
'0000000000000000000000000f190000' '0000000000000000000000045ffef330' '0000000000000000000000055fffccc0' '000411155113fffffffff777fffc0000' '00000454440ffffffffffffffc000000' '000000000002ffcccccccccff3000000' '00000000002bc000000000008ec30000' '00000000180d00000000000000814000'
'0000000000000000000000010f190000' '0000000000000000000000015ffef330' '0000000000000000000000015fffccc0' '000411155113fffffffff777fffc0000' '00000454440ffffffffffffffc000000' '000000000002ffcccccccccff3000000' '00000000488bc00000000000acc30000' '00000000004000000000000900004000'
'0000000000000000000000000f190000' '0000000000000000000000045ffef330' '0000000000000000000000055fffccc0' '000100115113fffffffff777fffc0000' '00004455440ffffffffffffffc000000' '000000000000ffecccccccff30000000' '00000000003ce80000000028c3000000' '0000000004040000000008100c100000'
'0000000000000000000000000f190000' '0000000000000000000000015ffef330' '0000000000000000000000055fffccc0' '000000115113fffffffff777fffc0000' '00001554440ffffffffffffffc000000' '000000000000ffecccccccff00000000' '0000000013ccc2000000028f00000000' '00000000000008400000900c10000000'
'0000000000000000000000000f190000' '0000000000000000000000045ffef330' '0000000000000000000000055fffccc0' '0000000001113ffffffff777fffc0000' '000000155540ffffffffffffffc00000' '0000000440000ffeeccccccff0000000' '000000000003cca00000122f80000000' '0000000000040040000000d000000000'
)
FRAME_COUNT=15

# ── 單字元的 hex → 上下像素強度 ──────────────────────────────────
# hex v (0–15): top = v/4, bottom = v%4  (各 0–3)
#
# ── 渲染一幀 ───────────────────────────────────────────────────────
# 眼睛像素: 原始為 pixel[3][27] → cell row 1 的「下」半像素，col 27。
# 平時半瞇 (shade 1)，blink 時 shade 2，>=2 才覆蓋 — 確保眼睛始終可見。
EYE_ROW=1; EYE_COL=27
#
# render_frame <frame_index> <color:1|0> <blink:1|0>
# 每個 cell row = 一行文字; 每個 hex = 一個半角字元; run-length 壓縮 fg/bg。
render_frame() {
    local frame=$(( $1 % FRAME_COUNT ))
    local color="${2:-1}"
    local blink="${3:-0}"
    local base=$(( frame * 8 ))
    local r row line active_fg active_bg c ch v top bottom char want_fg want_bg

    for (( r = 0; r < 8; r++ )); do
        row="${ROWS[base + r]}"
        line=""
        active_fg=-1
        active_bg=-1
        for (( c = 0; c < ${#row}; c++ )); do
            ch="${row:c:1}"
            v=$(( 16#$ch ))
            top=$(( v / 4 ))
            bottom=$(( v % 4 ))
            # 眼睛: 半瞇 (shade1) 或 blink (shade2)，僅在原強度 >=2 時覆蓋
            if [ "$r" -eq "$EYE_ROW" ] && [ "$c" -eq "$EYE_COL" ] && [ "$bottom" -ge 2 ]; then
                if [ "$blink" = 1 ]; then bottom=2; else bottom=1; fi
            fi

            if [ "$color" != 1 ]; then
                if   [ "$top" -gt 0 ] && [ "$bottom" -gt 0 ]; then line+="$BLOCK_FULL"
                elif [ "$top" -gt 0 ]; then line+="$BLOCK_TOP"
                elif [ "$bottom" -gt 0 ]; then line+="$BLOCK_BOTTOM"
                else line+=" "; fi
                continue
            fi

            # 透明 (上下皆空)
            if [ "$top" -eq 0 ] && [ "$bottom" -eq 0 ]; then
                if [ "$active_bg" -ne -1 ]; then line+="$BG_RESET"; active_bg=-1; fi
                line+=" "
                continue
            fi

            want_bg=-1
            if [ "$top" -gt 0 ] && [ "$bottom" -gt 0 ]; then
                if [ "$top" -eq "$bottom" ]; then
                    char="$BLOCK_FULL"; want_fg="$top"
                else
                    char="$BLOCK_TOP"; want_fg="$top"; want_bg="$bottom"
                fi
            elif [ "$top" -gt 0 ]; then
                char="$BLOCK_TOP"; want_fg="$top"
            else
                char="$BLOCK_BOTTOM"; want_fg="$bottom"
            fi

            if [ "$want_fg" -ne "$active_fg" ]; then line+="${FG[$want_fg]}"; active_fg="$want_fg"; fi
            if [ "$want_bg" -ne "$active_bg" ]; then
                if [ "$want_bg" -eq -1 ]; then line+="$BG_RESET"; else line+="${BG[$want_bg]}"; fi
                active_bg="$want_bg"
            fi
            line+="$char"
        done

        # 去尾端空白; 開頭空白用盲文空格錨定 (保留縮排)
        line="${line%"${line##*[! ]}"}"
        [ "${line:0:1}" = " " ] && line="${BLANK_ANCHOR}${line:1}"
        [ "$color" = 1 ] && case "$line" in *$'\e['*) line+="$RESET";; esac
        printf '%s\n' "$line"
    done
}

# ── token 速率 → 馬腿 fps (對齊 tokenRateToLegFps) ─────────────────
# <5 tok/s → 0 (站立); 否則 1.5 + sqrt(clamp((r-20)/880,0,1)) * 22.5
leg_fps() {
    awk -v r="$1" 'BEGIN{
        if (r < 5) { print 0; exit }
        n = (r - 20) / (900 - 20);
        if (n < 0) n = 0; if (n > 1) n = 1;
        printf "%.4f", 1.5 + sqrt(n) * (24 - 1.5);
    }'
}

# ── 子命令 ─────────────────────────────────────────────────────────
cmd="${1:-frame}"
case "$cmd" in
    frame)
        render_frame "${2:-0}" 1
        ;;
    all)
        for (( f = 0; f < FRAME_COUNT; f++ )); do
            printf '── frame %d ──\n' "$f"
            render_frame "$f" 1
        done
        ;;
    anim)
        rate="${2:-450}"
        secs="${3:-6}"
        render_fps=20
        delay=$(awk -v f="$render_fps" 'BEGIN{printf "%.3f", 1/f}')
        fps=$(leg_fps "$rate")
        total=$(( render_fps * secs ))
        legphase=0
        printf '\e[?25l'                       # 隱藏游標
        trap 'printf "\e[?25h\n"' EXIT INT
        for (( i = 0; i < total; i++ )); do
            idx=$(awk -v p="$legphase" -v n="$FRAME_COUNT" 'BEGIN{printf "%d", (int(p) % n)}')
            printf '\e[H\e[2J'                  # 游標歸位 + 清屏
            printf 'rate=%s tok/s  legFps=%s  frame=%d\n\n' "$rate" "$fps" "$idx"
            render_frame "$idx" 1
            legphase=$(awk -v p="$legphase" -v fps="$fps" -v rf="$render_fps" 'BEGIN{printf "%.4f", p + fps/rf}')
            sleep "$delay"
        done
        ;;
    tick)
        # statusline 模式 PoC: 用狀態檔在多次獨立呼叫間維持動畫
        rate="${2:-450}"
        state_dir="${TMPDIR:-/tmp}/horse-poc"
        mkdir -p "$state_dir"
        state_file="$state_dir/state"
        now=$(date +%s)
        legphase=0; updated=$now
        if [ -f "$state_file" ]; then
            # shellcheck disable=SC1090
            IFS=' ' read -r legphase updated < "$state_file" 2>/dev/null || true
        fi
        delta=$(( now - updated )); [ "$delta" -lt 0 ] && delta=0; [ "$delta" -gt 4 ] && delta=4
        fps=$(leg_fps "$rate")
        # frame step cap = 4 (與 15 互質)，閒置歸 0
        legphase=$(awk -v p="$legphase" -v fps="$fps" -v d="$delta" -v n="$FRAME_COUNT" 'BEGIN{
            if (fps == 0) { print 0; exit }
            step = fps * d; if (step > 4) step = 4;
            v = p + step; printf "%.4f", v - int(v/n)*n;
        }')
        printf '%s %s\n' "$legphase" "$now" > "$state_file"
        idx=$(awk -v p="$legphase" -v n="$FRAME_COUNT" 'BEGIN{printf "%d", (int(p) % n)}')
        render_frame "$idx" 1
        ;;
    *)
        printf 'usage: %s {frame [N]|all|anim [rate] [secs]|tick [rate]}\n' "$0" >&2
        exit 2
        ;;
esac
