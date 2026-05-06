#!/bin/bash

# =====================================================================
# 預期效果範例 (Expected Output Example):
#
# [單行顯示 Single Line] (當終端機夠寬時):
# 📁 project_dir › 🌿 feat-008a [+2|-1] │ Opus 4.7 · effort: high │ 96k/200k (▓▓▓▓▓░░░░░ 48%) · 5h: ▓▓░░░░░░░░ 20% @15:00 · 7d: ▓▓▓▓▓░░░░░ 50% @Apr 24
# 
# [雙行顯示 Two Lines] (當資訊字串長度超過終端機寬度時自動折行):
# 📁 project_dir › 🌿 feat-008a [+2|-1] │ Opus 4.7 · effort: high
# └─ 96k/200k (▓▓▓▓▓░░░░░ 48%) · 5h: ▓▓░░░░░░░░ 20% @15:00 · 7d: ▓▓▓▓▓░░░░░ 50% @Apr 24 · extra: enabled
# =====================================================================

set -f  # disable globbing
VERSION="1.0.0"

input=$(cat)
now=$(date +%s)

if [ -z "$input" ]; then
    printf "Claude"
    exit 0
fi

# ANSI colors matching oh-my-posh theme (Optimized for Dark Terminal)
blue='\033[38;2;80;180;255m'
orange='\033[38;2;255;170;80m'
green='\033[38;2;100;255;100m'
cyan='\033[38;2;100;220;255m'
red='\033[38;2;255;100;100m'
yellow='\033[38;2;255;230;80m'
white='\033[38;2;240;240;240m'
dim='\033[2m'
reset='\033[0m'
bright_red='\033[38;2;255;50;50m'
gray='\033[38;2;140;140;140m'

# Format token counts (e.g., 50k / 200k)
format_tokens() {
    local num=$1
    if [ "$num" -ge 1000000 ]; then
        awk "BEGIN {printf \"%.1fm\", $num / 1000000}"
    elif [ "$num" -ge 1000 ]; then
        awk "BEGIN {printf \"%.0fk\", $num / 1000}"
    else
        printf "%d" "$num"
    fi
}

# Format number with commas (e.g., 134,938)
format_commas() {
    printf "%'d" "$1"
}

# Return color escape based on usage percentage
usage_color() {
    local pct=$1
    if [ "$pct" -ge 90 ]; then echo "$red"
    elif [ "$pct" -ge 70 ]; then echo "$orange"
    elif [ "$pct" -ge 50 ]; then echo "$yellow"
    else echo "$green"
    fi
}

# Generate visual progress bar (e.g., ▓▓▓░░░░)
generate_bar() {
    local pct=$1
    local bar_width=${2:-10}
    local filled=$((pct * bar_width / 100))
    local empty=$((bar_width - filled))
    local bar=""
    local fill=""
    local pad=""
    
    [ "$filled" -gt 0 ] && printf -v fill "%${filled}s" && bar="${fill// /▓}"
    [ "$empty" -gt 0 ] && printf -v pad "%${empty}s" && bar="${bar}${pad// /░}"
    
    echo "$bar"
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

# ===== Extract data from JSON =====
model_name=$(echo "$input" | jq -r '.model.display_name // "Claude"')

# Context window & Token usage
size=$(echo "$input" | jq -r '.context_window.context_window_size // 200000')
[ "$size" -eq 0 ] 2>/dev/null && size=200000

input_tokens=$(echo "$input" | jq -r '.context_window.current_usage.input_tokens // 0')
cache_create=$(echo "$input" | jq -r '.context_window.current_usage.cache_creation_input_tokens // 0')
cache_read=$(echo "$input" | jq -r '.context_window.current_usage.cache_read_input_tokens // 0')
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
cwd=$(echo "$input" | jq -r '.cwd // empty' | sed 's/\\/\//g')
if [ -n "$cwd" ]; then
    display_dir="${cwd##*/}"
    git_branch=$(git -C "${cwd}" rev-parse --abbrev-ref HEAD 2>/dev/null)
    out+="📁 ${cyan}${display_dir}${reset}"
    
    if [ -n "$git_branch" ]; then
        out+="${arrow}🌿 ${green}${git_branch}${reset}"
        # Git 異動狀態 [+2|-1]
        git_stat=$(git -C "${cwd}" diff --numstat 2>/dev/null | awk '{a+=$1; d+=$2} END {if (a+d>0) printf "+%d -%d", a, d}')
        if [ -n "$git_stat" ]; then
            stat_a="${git_stat%% *}"
            stat_d="${git_stat##* }"
            out+=" ${dim}[${green}${stat_a}${dim}|${red}${stat_d}${dim}]${reset}"
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
token_color=$(usage_color "$pct_used")
token_bar=$(generate_bar "$pct_used" 10) # 統一使用長度 10 的 Bar
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
builtin_five_hour_pct=$(echo "$input" | jq -r '.rate_limits.five_hour.used_percentage // empty')
builtin_five_hour_reset=$(echo "$input" | jq -r '.rate_limits.five_hour.resets_at // empty')
builtin_seven_day_pct=$(echo "$input" | jq -r '.rate_limits.seven_day.used_percentage // empty')
builtin_seven_day_reset=$(echo "$input" | jq -r '.rate_limits.seven_day.resets_at // empty')

if [ -n "$builtin_five_hour_pct" ] || [ -n "$builtin_seven_day_pct" ]; then use_builtin=true; fi

claude_config_dir_hash=$(echo -n "$claude_config_dir" | shasum -a 256 2>/dev/null || echo -n "$claude_config_dir" | sha256sum 2>/dev/null | cut -c1-8)
cache_file="/tmp/claude/statusline-usage-cache-${claude_config_dir_hash}.json"
cache_max_age=60
mkdir -p /tmp/claude

needs_refresh=true
usage_data=""

if ! $use_builtin; then
    if [ -f "$cache_file" ] && [ -s "$cache_file" ]; then
        cache_mtime=$(stat -c %Y "$cache_file" 2>/dev/null || stat -f %m "$cache_file" 2>/dev/null)
        cache_age=$(( $(date +%s) - cache_mtime ))
        if [ "$cache_age" -lt "$cache_max_age" ]; then needs_refresh=false; fi
        usage_data=$(cat "$cache_file" 2>/dev/null)
    fi
    if $needs_refresh; then
        touch "$cache_file"
        token=$(get_oauth_token)
        if [ -n "$token" ] && [ "$token" != "null" ]; then
            response=$(curl -s --max-time 10 -H "Accept: application/json" -H "Content-Type: application/json" -H "Authorization: Bearer $token" -H "anthropic-beta: oauth-2025-04-20" -H "User-Agent: claude-code/2.1.34" "https://api.anthropic.com/api/oauth/usage" 2>/dev/null)
            if [ -n "$response" ] && echo "$response" | jq -e '.five_hour' >/dev/null 2>&1; then
                usage_data="$response"
                echo "$response" > "$cache_file"
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
# 取得 session_id，若無則用 cwd 雜湊
session_id=$(echo "$input" | jq -r '.session_id // empty')
if [ -z "$session_id" ]; then
    _cwd_raw=$(echo "$input" | jq -r '.cwd // empty')
    session_id=$(echo -n "${_cwd_raw:-no-cwd}" | sha256sum 2>/dev/null | cut -c1-16 || echo -n "${_cwd_raw:-no-cwd}" | shasum -a 256 | cut -c1-16)
fi
session_hash=$(echo -n "$session_id" | sha256sum 2>/dev/null | cut -c1-16 || echo -n "$session_id" | shasum -a 256 | cut -c1-16)
cache_ttl_file="/tmp/claude/cache-ttl-${session_hash}.json"

signature="${input_tokens}:${cache_create}:${cache_read}"

state_started_at=""
state_signature=""
state_last_hit_rate=""
if [ -f "$cache_ttl_file" ] && [ -s "$cache_ttl_file" ]; then
    _raw_state=$(cat "$cache_ttl_file" 2>/dev/null)
    if echo "$_raw_state" | jq -e '.signature and .started_at' >/dev/null 2>&1; then
        state_signature=$(echo "$_raw_state" | jq -r '.signature // empty')
        state_started_at=$(echo "$_raw_state" | jq -r '.started_at // empty')
        state_last_hit_rate=$(echo "$_raw_state" | jq -r '.last_hit_rate // empty')
    fi
fi

has_usage=false
( [ "$input_tokens" -gt 0 ] || [ "$cache_create" -gt 0 ] || [ "$cache_read" -gt 0 ] ) && has_usage=true

hit_rate=""
if $has_usage; then
    _total_for_cache=$(( input_tokens + cache_create + cache_read ))
    if [ "$_total_for_cache" -gt 0 ]; then
        hit_rate=$(awk "BEGIN {printf \"%.0f\", $cache_read * 100 / $_total_for_cache}")
    else
        hit_rate="0"
    fi
fi

new_started_at="$state_started_at"
new_last_hit_rate="${state_last_hit_rate}"

if $has_usage && [ "$signature" != "$state_signature" ]; then
    new_started_at="$now"
    new_last_hit_rate="$hit_rate"
    printf '{"signature":"%s","started_at":%s,"last_hit_rate":"%s"}' \
        "$signature" "$now" "$hit_rate" > "$cache_ttl_file"
elif [ -z "$state_started_at" ] && ! $has_usage; then
    :
elif [ -z "$state_started_at" ] && $has_usage; then
    new_started_at="$now"
    new_last_hit_rate="$hit_rate"
    printf '{"signature":"%s","started_at":%s,"last_hit_rate":"%s"}' \
        "$signature" "$now" "$hit_rate" > "$cache_ttl_file"
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
        five_hour_pct=$(printf "%.0f" "$builtin_five_hour_pct")
        five_hour_color=$(usage_color "$five_hour_pct")
        five_hour_bar=$(generate_bar "$five_hour_pct" 10)
        limit_block+="${dim}5h: ${reset}${five_hour_color}${five_hour_bar} ${five_hour_pct}%${reset}"
        if [ -n "$builtin_five_hour_reset" ] && [ "$builtin_five_hour_reset" != "null" ]; then
            five_hour_reset=$(date -j -r "$builtin_five_hour_reset" +"%H:%M" 2>/dev/null || date -d "@$builtin_five_hour_reset" +"%H:%M" 2>/dev/null)
            [ -n "$five_hour_reset" ] && limit_block+=" ${dim}@${five_hour_reset}${reset}"
        fi
    fi
    if [ -n "$builtin_seven_day_pct" ]; then
        seven_day_pct=$(printf "%.0f" "$builtin_seven_day_pct")
        seven_day_color=$(usage_color "$seven_day_pct")
        seven_day_bar=$(generate_bar "$seven_day_pct" 10)
        [ -n "$limit_block" ] && limit_block+="${sep_sub}"
        limit_block+="${dim}7d: ${reset}${seven_day_color}${seven_day_bar} ${seven_day_pct}%${reset}"
        if [ -n "$builtin_seven_day_reset" ] && [ "$builtin_seven_day_reset" != "null" ]; then
            seven_day_reset=$(date -j -r "$builtin_seven_day_reset" +"%b %-d, %H:%M" 2>/dev/null || date -d "@$builtin_seven_day_reset" +"%b %-d, %H:%M" 2>/dev/null)
            [ -n "$seven_day_reset" ] && limit_block+=" ${dim}@${seven_day_reset}${reset}"
        fi
    fi
elif [ -n "$usage_data" ] && echo "$usage_data" | jq -e '.five_hour' >/dev/null 2>&1; then
    [ -n "$limit_block" ] && limit_block+="${sep_sub}"
    five_hour_pct=$(echo "$usage_data" | jq -r '.five_hour.utilization // 0' | awk '{printf "%.0f", $1}')
    five_hour_reset=$(format_reset_time "$(echo "$usage_data" | jq -r '.five_hour.resets_at // empty')" "time")
    five_hour_bar=$(generate_bar "$five_hour_pct" 10)
    limit_block+="${dim}5h: ${reset}$(usage_color "$five_hour_pct")${five_hour_bar} ${five_hour_pct}%${reset}"
    [ -n "$five_hour_reset" ] && limit_block+=" ${dim}@${five_hour_reset}${reset}"

    seven_day_pct=$(echo "$usage_data" | jq -r '.seven_day.utilization // 0' | awk '{printf "%.0f", $1}')
    seven_day_reset=$(format_reset_time "$(echo "$usage_data" | jq -r '.seven_day.resets_at // empty')" "datetime")
    seven_day_bar=$(generate_bar "$seven_day_pct" 10)
    [ -n "$limit_block" ] && limit_block+="${sep_sub}"
    limit_block+="${dim}7d: ${reset}$(usage_color "$seven_day_pct")${seven_day_bar} ${seven_day_pct}%${reset}"
    [ -n "$seven_day_reset" ] && limit_block+=" ${dim}@${seven_day_reset}${reset}"

    extra_enabled=$(echo "$usage_data" | jq -r '.extra_usage.is_enabled // false')
    if [ "$extra_enabled" = "true" ]; then
        extra_pct=$(echo "$usage_data" | jq -r '.extra_usage.utilization // 0' | awk '{printf "%.0f", $1}')
        extra_used=$(echo "$usage_data" | jq -r '.extra_usage.used_credits // 0' | LC_NUMERIC=C awk '{printf "%.2f", $1/100}')
        extra_limit=$(echo "$usage_data" | jq -r '.extra_usage.monthly_limit // 0' | LC_NUMERIC=C awk '{printf "%.2f", $1/100}')
        [ -n "$limit_block" ] && limit_block+="${sep_sub}"
        if [ -n "$extra_used" ] && [ -n "$extra_limit" ] && [[ "$extra_used" != *'$'* ]] && [[ "$extra_limit" != *'$'* ]]; then
            limit_block+="${dim}extra: ${reset}$(usage_color "$extra_pct")\$${extra_used}/\$${extra_limit}${reset}"
        else
            limit_block+="${dim}extra: ${reset}${green}enabled${reset}"
        fi
    fi
else
    [ -n "$limit_block" ] && limit_block+="${sep_sub}"
    limit_block+="${dim}5h: -${reset}${sep_sub}${dim}7d: -${reset}"
fi

# ===== 6. 動態折行處理 (Dynamic line break if too long) =====
# 剃除 ANSI 控制碼來計算真實長度
clean_out=$(echo "$out" | sed -E 's/\x1B\[[0-9;]*[a-zA-Z]//g')
clean_limit=$(echo "$limit_block" | sed -E 's/\x1B\[[0-9;]*[a-zA-Z]//g')

# 取得終端機寬度 (預設 100)
term_width=${COLUMNS:-$(tput cols 2>/dev/null || echo 100)}
total_visual_len=$((${#clean_out} + ${#clean_limit} + 5))

final_output=""
if [ "$total_visual_len" -gt "$term_width" ] && [ -n "$limit_block" ]; then
    # 超過寬度，強制分為兩行顯示，並加上層級線條
    final_output="${out}\n${dim}└─${reset} ${limit_block}"
else
    # 夠寬，單行顯示
    [ -n "$limit_block" ] && final_output="${out}${sep_main}${limit_block}" || final_output="${out}"
fi

# ===== Update check =====
version_cache_file="/tmp/claude/statusline-version-cache.json"
version_cache_max_age=86400

version_needs_refresh=true
version_data=""

if [ -f "$version_cache_file" ]; then
    vc_mtime=$(stat -c %Y "$version_cache_file" 2>/dev/null || stat -f %m "$version_cache_file" 2>/dev/null)
    if [ $(( $(date +%s) - vc_mtime )) -lt "$version_cache_max_age" ]; then version_needs_refresh=false; fi
    version_data=$(cat "$version_cache_file" 2>/dev/null)
fi

if $version_needs_refresh; then
    touch "$version_cache_file" 2>/dev/null
    vc_response=$(curl -s --max-time 5 -H "Accept: application/vnd.github+json" "https://api.github.com/repos/gn00678465/StatusLine/releases/latest" 2>/dev/null)
    if [ -n "$vc_response" ] && echo "$vc_response" | jq -e '.tag_name' >/dev/null 2>&1; then
        version_data="$vc_response"
        echo "$vc_response" > "$version_cache_file"
    fi
fi

update_line=""
if [ -n "$version_data" ]; then
    latest_tag=$(echo "$version_data" | jq -r '.tag_name // empty')
    if [ -n "$latest_tag" ] && version_gt "$latest_tag" "$VERSION"; then
        update_line="\n${dim}Update available: ${latest_tag} → https://github.com/gn00678465/StatusLine${reset}"
    fi
fi

# Final Output
printf "%b" "$final_output$update_line"
exit 0