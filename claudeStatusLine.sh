#!/bin/bash

# =====================================================================
# 預期效果範例 (Expected Output Example):
#
# [單行顯示 Single Line] (當終端機夠寬時):
# 📁 project_dir › 🌿 feat-008a [+2|-1] │ Opus 4.7 · effort: high │ 96k/200k (▓▓▓▓▓░░░░░ 48%) · Cache 94% 56:41 · 5h: ▓▓░░░░░░░░ 20% @15:00 · 7d: ▓▓▓▓▓░░░░░ 50% @Apr 24, 08:00 · extra: $1.23/$10.00
#
# [雙行顯示 Two Lines] (當資訊字串長度超過終端機寬度時自動折行):
# 📁 project_dir › 🌿 feat-008a [+2|-1] │ Opus 4.7 · effort: high
# └─ 96k/200k (▓▓▓▓▓░░░░░ 48%) · Cache 94% 56:41 · 5h: ▓▓░░░░░░░░ 20% @15:00 · 7d: ▓▓▓▓▓░░░░░ 50% @Apr 24, 08:00 · extra: $1.23/$10.00
#
# 區塊說明:
#   Cache <hit%> <MM:SS>  — 命中率 = cache_read / (input + cache_creation + cache_read)
#                          綠 ≥50% / 灰 <50%；TTL 從上次響應倒數 1 小時
#                          顏色: 0-20m 綠 / 20-40m 黃 / 40-55m 紅 / 最後 5m 閃紅 / 過期 exp 灰
# =====================================================================

set -f  # disable globbing
VERSION="1.1.1"

input=$(cat)
now=$(date +%s)

if [ -z "$input" ]; then
    printf "Claude"
    exit 0
fi

# ANSI colors — $'...' embeds real ESC bytes so the final printf can use %s
# (which won't interpret backslash escapes from any user-controlled string).
blue=$'\033[38;2;80;180;255m'
orange=$'\033[38;2;255;170;80m'
green=$'\033[38;2;100;255;100m'
cyan=$'\033[38;2;100;220;255m'
red=$'\033[38;2;255;100;100m'
yellow=$'\033[38;2;255;230;80m'
white=$'\033[38;2;240;240;240m'
dim=$'\033[2m'
reset=$'\033[0m'
bright_red=$'\033[38;2;255;50;50m'
gray=$'\033[38;2;140;140;140m'

# Format token counts (e.g., 50k / 200k) — pure bash arithmetic
format_tokens() {
    local num=$1
    if [ "$num" -ge 1000000 ]; then
        local whole=$(( num / 1000000 ))
        local frac=$(( (num % 1000000 + 50000) / 100000 ))
        if [ "$frac" -ge 10 ]; then whole=$((whole+1)); frac=0; fi
        printf "%d.%dm" "$whole" "$frac"
    elif [ "$num" -ge 1000 ]; then
        printf "%dk" $(( (num + 500) / 1000 ))
    else
        printf "%d" "$num"
    fi
}

# Inline variants — write to global $_uc / $_gb instead of echoing, avoiding $() subshells
usage_color_inline() {
    local pct=$1
    if [ "$pct" -ge 90 ]; then _uc=$red
    elif [ "$pct" -ge 70 ]; then _uc=$orange
    elif [ "$pct" -ge 50 ]; then _uc=$yellow
    else _uc=$green
    fi
}

generate_bar_inline() {
    local pct=$1
    local bar_width=${2:-10}
    local filled=$((pct * bar_width / 100))
    local empty=$((bar_width - filled))
    local fill="" pad=""
    _gb=""
    [ "$filled" -gt 0 ] && printf -v fill "%${filled}s" && _gb="${fill// /▓}"
    [ "$empty" -gt 0 ] && printf -v pad "%${empty}s" && _gb+="${pad// /░}"
}

# Resolve config directory
claude_config_dir="${CLAUDE_CONFIG_DIR:-$HOME/.claude}"

# Return 0 (true) if $1 > $2 using semantic versioning
version_gt() {
    local a="${1#v}" b="${2#v}"
    local IFS='.'
    read -r a1 a2 a3 <<< "$a"
    read -r b1 b2 b3 <<< "$b"
    a1=${a1:-0}; a2=${a2:-0}; a3=${a3:-0}
    b1=${b1:-0}; b2=${b2:-0}; b3=${b3:-0}
    [ "$a1" -gt "$b1" ] 2>/dev/null && return 0
    [ "$a1" -lt "$b1" ] 2>/dev/null && return 1
    [ "$a2" -gt "$b2" ] 2>/dev/null && return 0
    [ "$a2" -lt "$b2" ] 2>/dev/null && return 1
    [ "$a3" -gt "$b3" ] 2>/dev/null && return 0
    return 1
}

# ===== Single jq pass extracts everything we need from input =====
# `tonumber? // 0` 強制數字欄位即使遇到字串／物件也回 0，杜絕 arithmetic injection。
# 字串欄位透過 gsub 移除控制字元（含 ESC \e、CR、LF），避免 terminal escape 注入。
mapfile -t _inp < <(jq -r '
  def safe_str: tostring | gsub("[\u0000-\u001f\u007f]"; "");
  (.model.display_name // "Claude" | safe_str),
  (.context_window.context_window_size // 200000 | tonumber? // 200000),
  (.context_window.current_usage.input_tokens // 0 | tonumber? // 0),
  (.context_window.current_usage.cache_creation_input_tokens // 0 | tonumber? // 0),
  (.context_window.current_usage.cache_read_input_tokens // 0 | tonumber? // 0),
  (.session_id // "" | safe_str),
  (.cwd // "" | safe_str),
  (.rate_limits.five_hour.used_percentage // "" | tostring),
  (.rate_limits.five_hour.resets_at // "" | safe_str),
  (.rate_limits.seven_day.used_percentage // "" | tostring),
  (.rate_limits.seven_day.resets_at // "" | safe_str)
' <<< "$input" 2>/dev/null)
# Strip trailing CR (jq on Git Bash emits CRLF)
for _i in "${!_inp[@]}"; do _inp[_i]=${_inp[_i]%$'\r'}; done

# Defense in depth: 即使 jq 邏輯被繞過，bash 端再強制數字驗證；任何非純數字 → 0
_num() { [[ "$1" =~ ^[0-9]+$ ]] && echo "$1" || echo 0; }

model_name=${_inp[0]:-Claude}
size=$(_num "${_inp[1]:-200000}")
[ "$size" -eq 0 ] && size=200000
input_tokens=$(_num "${_inp[2]:-0}")
cache_create=$(_num "${_inp[3]:-0}")
cache_read=$(_num "${_inp[4]:-0}")
session_id=${_inp[5]}
cwd_raw=${_inp[6]}
builtin_five_hour_pct=${_inp[7]}
builtin_five_hour_reset=${_inp[8]}
builtin_seven_day_pct=${_inp[9]}
builtin_seven_day_reset=${_inp[10]}

current=$(( input_tokens + cache_create + cache_read ))

used_tokens=$(format_tokens $current)
total_tokens=$(format_tokens $size)

if [ "$size" -gt 0 ]; then
    pct_used=$(( current * 100 / size ))
else
    pct_used=0
fi

# Check reasoning effort
settings_path="$claude_config_dir/settings.json"
effort_level="medium"
if [ -n "$CLAUDE_CODE_EFFORT_LEVEL" ]; then
    effort_level="$CLAUDE_CODE_EFFORT_LEVEL"
elif [ -f "$settings_path" ]; then
    effort_val=$(jq -r '.effortLevel // empty' "$settings_path" 2>/dev/null)
    [ -n "$effort_val" ] && effort_level="$effort_val"
fi

# ===== 視覺符號與排版設定 =====
out=""
sep_main=" ${dim}│${reset} "  # 主區塊分隔線
sep_sub=" ${dim}·${reset} "   # 次屬性分隔點
arrow=" ${dim}›${reset} "     # 層級遞進符號

# 1. Workspace (Dir › Branch)
cwd=${cwd_raw//\\//}
if [ -n "$cwd" ]; then
    display_dir="${cwd##*/}"
    git_branch=$(git -C "${cwd}" rev-parse --abbrev-ref HEAD 2>/dev/null)
    out+="📁 ${cyan}${display_dir}${reset}"

    if [ -n "$git_branch" ]; then
        out+="${arrow}🌿 ${green}${git_branch}${reset}"
        # Git 異動狀態 [+2|-1] — pure bash sum
        _added=0; _deleted=0
        while read -r _a _d _; do
            [[ "$_a" =~ ^[0-9]+$ ]] && _added=$((_added + _a))
            [[ "$_d" =~ ^[0-9]+$ ]] && _deleted=$((_deleted + _d))
        done < <(git -C "${cwd}" diff --numstat 2>/dev/null)
        if [ $((_added + _deleted)) -gt 0 ]; then
            out+=" ${dim}[${green}+${_added}${dim}|${red}-${_deleted}${dim}]${reset}"
        fi
    fi
    out+="${sep_main}"
fi

# 2. Model & Effort
out+="${blue}${model_name}${reset}"
out+="${sep_sub}"
out+="${dim}effort: ${reset}"
case "$effort_level" in
    low)    out+="${dim}${effort_level}${reset}" ;;
    medium) out+="${yellow}med${reset}" ;;
    max)    out+="${red}${effort_level}${reset}" ;;
    *)      out+="${orange}${effort_level}${reset}" ;;
esac
# 這裡不再加入 sep_main，將由後續折行邏輯決定

# 3. Context Window Usage (將作為 limit_block 的起點)
usage_color_inline "$pct_used"; token_color=$_uc
generate_bar_inline "$pct_used" 10; token_bar=$_gb
context_block="${white}${used_tokens}${reset}${dim}/${total_tokens}${reset} ${dim}(${token_color}${token_bar} ${pct_used}%${reset}${dim})${reset}"


# ===== OAuth & Rate Limits Fetching =====
get_oauth_token() {
    local token=""
    if [ -n "$CLAUDE_CODE_OAUTH_TOKEN" ]; then echo "$CLAUDE_CODE_OAUTH_TOKEN"; return 0; fi
    if command -v security >/dev/null 2>&1; then
        local keychain_svc="Claude Code-credentials"
        if [ -n "$CLAUDE_CONFIG_DIR" ]; then
            local dir_hash=$(echo -n "$CLAUDE_CONFIG_DIR" | shasum -a 256 | cut -c1-8)
            keychain_svc="Claude Code-credentials-${dir_hash}"
        fi
        local blob=$(security find-generic-password -s "$keychain_svc" -w 2>/dev/null)
        if [ -n "$blob" ]; then
            token=$(echo "$blob" | jq -r '.claudeAiOauth.accessToken // empty' 2>/dev/null)
            if [ -n "$token" ] && [ "$token" != "null" ]; then echo "$token"; return 0; fi
        fi
    fi
    local creds_file="${claude_config_dir}/.credentials.json"
    if [ -f "$creds_file" ]; then
        token=$(jq -r '.claudeAiOauth.accessToken // empty' "$creds_file" 2>/dev/null)
        if [ -n "$token" ] && [ "$token" != "null" ]; then echo "$token"; return 0; fi
    fi
    if command -v secret-tool >/dev/null 2>&1; then
        local blob=$(timeout 2 secret-tool lookup service "Claude Code-credentials" 2>/dev/null)
        if [ -n "$blob" ]; then
            token=$(echo "$blob" | jq -r '.claudeAiOauth.accessToken // empty' 2>/dev/null)
            if [ -n "$token" ] && [ "$token" != "null" ]; then echo "$token"; return 0; fi
        fi
    fi
    echo ""
}

use_builtin=false
if [ -n "$builtin_five_hour_pct" ] || [ -n "$builtin_seven_day_pct" ]; then use_builtin=true; fi

# Per-user cache dir avoids symlink attacks via shared /tmp/claude.
# Use $UID where set; fall back to a sanitized $USER; last resort 'anon'.
_uid=${UID:-${EUID:-${USER:-anon}}}
_uid=${_uid//[^a-zA-Z0-9_-]/_}
cache_dir="/tmp/claude-${_uid}"
umask 077
mkdir -p "$cache_dir" 2>/dev/null
chmod 700 "$cache_dir" 2>/dev/null

# Atomic write helper: writes to mktemp + rename, never follows symlinks
_atomic_write() {
    local target=$1 content=$2
    local tmp
    tmp=$(mktemp "${target}.XXXXXX") || return 1
    printf '%s' "$content" > "$tmp" && mv -f "$tmp" "$target"
}

claude_config_dir_hash=${claude_config_dir//[^a-zA-Z0-9]/_}
claude_config_dir_hash=${claude_config_dir_hash:0:64}
cache_file="${cache_dir}/usage-cache-${claude_config_dir_hash}.json"
cache_max_age=60

needs_refresh=true
usage_data=""

if ! $use_builtin; then
    if [ -f "$cache_file" ] && [ -s "$cache_file" ]; then
        cache_mtime=$(stat -c %Y "$cache_file" 2>/dev/null || stat -f %m "$cache_file" 2>/dev/null || echo 0)
        cache_age=$(( now - ${cache_mtime:-0} ))
        if [ "$cache_age" -lt "$cache_max_age" ]; then needs_refresh=false; fi
        usage_data=$(< "$cache_file")
    fi
    if $needs_refresh; then
        token=$(get_oauth_token)
        if [ -n "$token" ] && [ "$token" != "null" ]; then
            # Pass Authorization header via stdin --config to keep token out of argv
            # (visible in ps/proc otherwise). Other headers stay in argv since they're
            # not secrets. --connect-timeout caps DNS/TCP stalls separately.
            response=$(printf 'header = "Authorization: Bearer %s"\n' "$token" \
                | curl -s --config - \
                    --max-time 10 \
                    --connect-timeout 3 \
                    -H "Accept: application/json" \
                    -H "Content-Type: application/json" \
                    -H "anthropic-beta: oauth-2025-04-20" \
                    -H "User-Agent: claude-code/2.1.34" \
                    "https://api.anthropic.com/api/oauth/usage" 2>/dev/null)
            if [ -n "$response" ] && echo "$response" | jq -e '.five_hour' >/dev/null 2>&1; then
                usage_data="$response"
                _atomic_write "$cache_file" "$response"
            fi
        fi
    fi
fi

iso_to_epoch() {
    local iso_str="$1"
    local epoch=$(date -d "${iso_str}" +%s 2>/dev/null)
    if [ -n "$epoch" ]; then echo "$epoch"; return 0; fi
    local stripped="${iso_str%%.*}"; stripped="${stripped%%Z}"; stripped="${stripped%%+*}"; stripped="${stripped%%-[0-9][0-9]:[0-9][0-9]}"
    if [[ "$iso_str" == *"Z"* ]] || [[ "$iso_str" == *"+00:00"* ]] || [[ "$iso_str" == *"-00:00"* ]]; then
        epoch=$(env TZ=UTC date -j -f "%Y-%m-%dT%H:%M:%S" "$stripped" +%s 2>/dev/null)
    else
        epoch=$(date -j -f "%Y-%m-%dT%H:%M:%S" "$stripped" +%s 2>/dev/null)
    fi
    if [ -n "$epoch" ]; then echo "$epoch"; return 0; fi
    return 1
}

format_reset_time() {
    local iso_str="$1" style="$2"
    { [ -z "$iso_str" ] || [ "$iso_str" = "null" ]; } && return
    local epoch=$(iso_to_epoch "$iso_str")
    [ -z "$epoch" ] && return
    local formatted=""
    case "$style" in
        time)     formatted=$(date -d "@$epoch" +"%H:%M" 2>/dev/null || date -j -r "$epoch" +"%H:%M" 2>/dev/null) ;;
        datetime) formatted=$(date -d "@$epoch" +"%b %-d, %H:%M" 2>/dev/null || date -j -r "$epoch" +"%b %-d, %H:%M" 2>/dev/null) ;;
        *)        formatted=$(date -d "@$epoch" +"%b %-d" 2>/dev/null || date -j -r "$epoch" +"%b %-d" 2>/dev/null) ;;
    esac
    [ -n "$formatted" ] && echo "$formatted"
}

# ===== 4. Cache 命中率 + TTL 倒計時 (排在 5h 之前) =====
# session_id 已在 jq 端做過控制字元清洗；這裡只保留檔名安全字元
[ -z "$session_id" ] && session_id="${cwd:-no-cwd}"
session_hash=${session_id//[^a-zA-Z0-9_-]/_}
session_hash=${session_hash:0:32}
cache_ttl_file="${cache_dir}/cache-ttl-${session_hash}.json"

# Defense in depth: state_started_at 之後會進 $(( ... ))，一定要驗證為純數字
signature="${input_tokens}:${cache_create}:${cache_read}"

state_started_at=""
state_signature=""
state_last_hit_rate=""
if [ -f "$cache_ttl_file" ] && [ -s "$cache_ttl_file" ]; then
    # Single jq call: validate + extract all three fields, with control-char strip
    mapfile -t _state < <(jq -r '
        def safe_str: tostring | gsub("[ -]"; "");
        if (.signature and .started_at)
        then (.signature | safe_str),
             (.started_at | tonumber? // 0 | tostring),
             ((.last_hit_rate // "") | safe_str)
        else "","","" end
    ' < "$cache_ttl_file" 2>/dev/null)
    for _i in "${!_state[@]}"; do _state[_i]=${_state[_i]%$'\r'}; done
    state_signature=${_state[0]}
    state_started_at=$(_num "${_state[1]}")
    [ "$state_started_at" = "0" ] && state_started_at=""
    state_last_hit_rate=${_state[2]}
fi

has_usage=false
( [ "$input_tokens" -gt 0 ] || [ "$cache_create" -gt 0 ] || [ "$cache_read" -gt 0 ] ) && has_usage=true

hit_rate=""
if $has_usage; then
    _total_for_cache=$(( input_tokens + cache_create + cache_read ))
    if [ "$_total_for_cache" -gt 0 ]; then
        # Pure bash rounded percentage
        hit_rate=$(( (cache_read * 100 + _total_for_cache / 2) / _total_for_cache ))
    else
        hit_rate="0"
    fi
fi

new_started_at="$state_started_at"
new_last_hit_rate="${state_last_hit_rate}"

_write_ttl_state() {
    local content
    printf -v content '{"signature":"%s","started_at":%s,"last_hit_rate":"%s"}' \
        "$signature" "$now" "$hit_rate"
    _atomic_write "$cache_ttl_file" "$content"
}

if $has_usage && [ "$signature" != "$state_signature" ]; then
    new_started_at="$now"
    new_last_hit_rate="$hit_rate"
    _write_ttl_state
elif [ -z "$state_started_at" ] && ! $has_usage; then
    :
elif [ -z "$state_started_at" ] && $has_usage; then
    new_started_at="$now"
    new_last_hit_rate="$hit_rate"
    _write_ttl_state
fi

display_hit_rate="${hit_rate:-$new_last_hit_rate}"

cache_block=""
if [ -n "$new_started_at" ] && [ "$new_started_at" -gt 0 ] 2>/dev/null; then
    _elapsed=$(( now - new_started_at ))
    _ttl_total=3600
    _remaining=$(( _ttl_total - _elapsed ))

    if [ "$_remaining" -le 0 ]; then
        ttl_str="exp"
        ttl_color="$gray"
    else
        _min=$(( _remaining / 60 ))
        _sec=$(( _remaining % 60 ))
        ttl_str=$(printf "%d:%02d" "$_min" "$_sec")

        if [ "$_remaining" -le 300 ]; then
            if [ $(( now % 2 )) -eq 0 ]; then
                ttl_color="$red"
            else
                ttl_color="$bright_red"
            fi
        elif [ "$_remaining" -le 1200 ]; then
            ttl_color="$red"
        elif [ "$_remaining" -le 2400 ]; then
            ttl_color="$yellow"
        else
            ttl_color="$green"
        fi
    fi

    if [ -n "$display_hit_rate" ]; then
        _hr_num=$(printf "%.0f" "$display_hit_rate" 2>/dev/null || echo "0")
        if [ "$_hr_num" -ge 50 ]; then
            hr_color="$green"
        else
            hr_color="$gray"
        fi
        cache_block="${dim}Cache ${reset}${hr_color}${display_hit_rate}%${reset} ${ttl_color}${ttl_str}${reset}"
    else
        cache_block="${dim}Cache ${reset}${ttl_color}${ttl_str}${reset}"
    fi
elif [ -n "$display_hit_rate" ]; then
    _hr_num=$(printf "%.0f" "$display_hit_rate" 2>/dev/null || echo "0")
    if [ "$_hr_num" -ge 50 ]; then
        hr_color="$green"
    else
        hr_color="$gray"
    fi
    cache_block="${dim}Cache ${reset}${hr_color}${display_hit_rate}%${reset}"
fi

# ===== 5. Rate Limits (加上進度條與 Context block 整合) =====
limit_block="${context_block}"
if [ -n "$cache_block" ]; then
    [ -n "$limit_block" ] && limit_block+="${sep_sub}"
    limit_block+="${cache_block}"
fi

if $use_builtin; then
    if [ -n "$builtin_five_hour_pct" ]; then
        [ -n "$limit_block" ] && limit_block+="${sep_sub}"
        printf -v five_hour_pct "%.0f" "$builtin_five_hour_pct"
        usage_color_inline "$five_hour_pct"; five_hour_color=$_uc
        generate_bar_inline "$five_hour_pct" 10; five_hour_bar=$_gb
        limit_block+="${dim}5h: ${reset}${five_hour_color}${five_hour_bar} ${five_hour_pct}%${reset}"
        if [ -n "$builtin_five_hour_reset" ] && [ "$builtin_five_hour_reset" != "null" ]; then
            five_hour_reset=$(date -j -r "$builtin_five_hour_reset" +"%H:%M" 2>/dev/null || date -d "@$builtin_five_hour_reset" +"%H:%M" 2>/dev/null)
            [ -n "$five_hour_reset" ] && limit_block+=" ${dim}@${five_hour_reset}${reset}"
        fi
    fi
    if [ -n "$builtin_seven_day_pct" ]; then
        printf -v seven_day_pct "%.0f" "$builtin_seven_day_pct"
        usage_color_inline "$seven_day_pct"; seven_day_color=$_uc
        generate_bar_inline "$seven_day_pct" 10; seven_day_bar=$_gb
        [ -n "$limit_block" ] && limit_block+="${sep_sub}"
        limit_block+="${dim}7d: ${reset}${seven_day_color}${seven_day_bar} ${seven_day_pct}%${reset}"
        if [ -n "$builtin_seven_day_reset" ] && [ "$builtin_seven_day_reset" != "null" ]; then
            seven_day_reset=$(date -j -r "$builtin_seven_day_reset" +"%b %-d, %H:%M" 2>/dev/null || date -d "@$builtin_seven_day_reset" +"%b %-d, %H:%M" 2>/dev/null)
            [ -n "$seven_day_reset" ] && limit_block+=" ${dim}@${seven_day_reset}${reset}"
        fi
    fi
elif [ -n "$usage_data" ]; then
    # Single jq pass — extracts everything plus formats reset times via strftime
    # jq does the date formatting (via localtime + manual month-name array to avoid
    # locale-dependent %b glyphs from MSYS jq). ISO cleanup makes +00:00 / fractional
    # seconds parseable by fromdateiso8601.
    mapfile -t _ud < <(jq -r '
      def clean_iso: sub("[.][0-9]+"; "") | sub("[+]00:00$"; "Z");
      def months: ["Jan","Feb","Mar","Apr","May","Jun","Jul","Aug","Sep","Oct","Nov","Dec"];
      def hm: . as $t | "\($t[3]|tostring|("0"+.)[-2:]):\($t[4]|tostring|("0"+.)[-2:])";
      if .five_hour then
        "OK",
        (.five_hour.utilization // 0 | round),
        (try (.five_hour.resets_at | clean_iso | fromdateiso8601 | localtime | hm) catch ""),
        (.seven_day.utilization // 0 | round),
        (try (.seven_day.resets_at | clean_iso | fromdateiso8601 | localtime | . as $t | "\(months[$t[1]]) \($t[2]), \($t|hm)") catch ""),
        (.extra_usage.is_enabled // false),
        (.extra_usage.utilization // 0 | round),
        ((.extra_usage.used_credits // 0) / 100),
        ((.extra_usage.monthly_limit // 0) / 100)
      else "FAIL" end
    ' <<< "$usage_data" 2>/dev/null)
    for _i in "${!_ud[@]}"; do _ud[_i]=${_ud[_i]%$'\r'}; done

    if [ "${_ud[0]}" = "OK" ]; then
        [ -n "$limit_block" ] && limit_block+="${sep_sub}"
        five_hour_pct=${_ud[1]}
        five_hour_reset=${_ud[2]}
        usage_color_inline "$five_hour_pct"; _color5=$_uc
        generate_bar_inline "$five_hour_pct" 10; _bar5=$_gb
        limit_block+="${dim}5h: ${reset}${_color5}${_bar5} ${five_hour_pct}%${reset}"
        [ -n "$five_hour_reset" ] && limit_block+=" ${dim}@${five_hour_reset}${reset}"

        seven_day_pct=${_ud[3]}
        seven_day_reset=${_ud[4]}
        usage_color_inline "$seven_day_pct"; _color7=$_uc
        generate_bar_inline "$seven_day_pct" 10; _bar7=$_gb
        limit_block+="${sep_sub}${dim}7d: ${reset}${_color7}${_bar7} ${seven_day_pct}%${reset}"
        [ -n "$seven_day_reset" ] && limit_block+=" ${dim}@${seven_day_reset}${reset}"

        if [ "${_ud[5]}" = "true" ]; then
            extra_pct=${_ud[6]}
            printf -v extra_used "%.2f" "${_ud[7]:-0}" 2>/dev/null || extra_used="0.00"
            printf -v extra_limit "%.2f" "${_ud[8]:-0}" 2>/dev/null || extra_limit="0.00"
            usage_color_inline "$extra_pct"; _colorE=$_uc
            limit_block+="${sep_sub}${dim}extra: ${reset}${_colorE}\$${extra_used}/\$${extra_limit}${reset}"
        fi
    fi
else
    [ -n "$limit_block" ] && limit_block+="${sep_sub}"
    limit_block+="${dim}5h: -${reset}${sep_sub}${dim}7d: -${reset}"
fi

# ===== 6. 動態折行處理 (Dynamic line break if too long) =====
# Bash extglob strip ANSI sequences (avoids two sed subprocesses)
shopt -s extglob
clean_out=${out//$'\033'\[*([0-9;])[a-zA-Z]/}
clean_limit=${limit_block//$'\033'\[*([0-9;])[a-zA-Z]/}

term_width=${COLUMNS:-100}
total_visual_len=$((${#clean_out} + ${#clean_limit} + 5))

final_output=""
if [ "$total_visual_len" -gt "$term_width" ] && [ -n "$limit_block" ]; then
    # 超過寬度，強制分為兩行顯示，並加上層級線條
    final_output="${out}"$'\n'"${dim}└─${reset} ${limit_block}"
else
    # 夠寬，單行顯示
    [ -n "$limit_block" ] && final_output="${out}${sep_main}${limit_block}" || final_output="${out}"
fi

# ===== Update check =====
# Per-user cache dir; latest_tag goes through safe_str + version-string regex
# to prevent terminal escape injection from a tampered cache file.
version_cache_file="${cache_dir}/statusline-version-cache.json"
version_cache_max_age=86400

version_needs_refresh=true
version_data=""

if [ -f "$version_cache_file" ]; then
    vc_mtime=$(stat -c %Y "$version_cache_file" 2>/dev/null || stat -f %m "$version_cache_file" 2>/dev/null || echo 0)
    if [ $(( now - ${vc_mtime:-0} )) -lt "$version_cache_max_age" ]; then version_needs_refresh=false; fi
    [ -s "$version_cache_file" ] && version_data=$(< "$version_cache_file")
fi

if $version_needs_refresh; then
    vc_response=$(curl -s --max-time 5 --connect-timeout 3 \
        -H "Accept: application/vnd.github+json" \
        "https://api.github.com/repos/gn00678465/StatusLine/releases/latest" 2>/dev/null)
    if [ -n "$vc_response" ] && echo "$vc_response" | jq -e '.tag_name' >/dev/null 2>&1; then
        version_data="$vc_response"
        _atomic_write "$version_cache_file" "$vc_response"
    fi
fi

update_line=""
if [ -n "$version_data" ]; then
    latest_tag=$(echo "$version_data" | jq -r '.tag_name // empty' | tr -dc 'a-zA-Z0-9.+-')
    if [ -n "$latest_tag" ] && version_gt "$latest_tag" "$VERSION"; then
        update_line=$'\n'"${dim}Update available: ${latest_tag} → https://github.com/gn00678465/StatusLine${reset}"
    fi
fi

# Final Output — %s instead of %b: user/cache-controlled strings (model_name,
# latest_tag, etc.) cannot inject ANSI sequences via backslash escapes because
# %s does not interpret \033 / \xHH / \uHHHH. Our own escapes were stored as
# real ESC bytes via $'...' at definition time, so colors still render.
printf "%s" "$final_output$update_line"
exit 0