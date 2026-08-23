//! Renders rate-limit status blocks.

use crate::oauth::UsageResponse;

use super::color::{DIM, GREEN, ORANGE, RED, RESET, YELLOW};
use super::meter::{render_meter, MeterStyle};

#[derive(Clone, Copy, Debug, Default)]
pub struct BuiltInLimits {
    pub five_hour: Option<BuiltInLimit>,
    pub seven_day: Option<BuiltInLimit>,
}

#[derive(Clone, Copy, Debug)]
pub struct BuiltInLimit {
    pub used_percentage: f64,
    pub resets_at: Option<u64>,
}

pub struct LimitRenderContext<'a> {
    pub builtin: BuiltInLimits,
    pub oauth: Option<&'a UsageResponse>,
    pub meter_style: MeterStyle,
}

pub trait LocalOffset {
    fn offset_seconds(&self, epoch_seconds: i64) -> i32;
}

pub struct SystemLocalOffset;

impl LocalOffset for SystemLocalOffset {
    fn offset_seconds(&self, epoch_seconds: i64) -> i32 {
        #[cfg(unix)]
        {
            system_local_offset_unix(epoch_seconds)
        }

        #[cfg(windows)]
        {
            system_local_offset_windows(epoch_seconds)
        }

        #[cfg(not(any(unix, windows)))]
        {
            let _unused_epoch_seconds = epoch_seconds;

            0
        }
    }
}

pub fn render_limits<O: LocalOffset>(context: LimitRenderContext<'_>, offset: &O) -> String {
    let mut blocks = Vec::new();

    if context.builtin.five_hour.is_some() || context.builtin.seven_day.is_some() {
        if let Some(five_hour) = context.builtin.five_hour {
            blocks.push(render_builtin_window(
                "5h",
                five_hour,
                ResetStyle::Time,
                context.meter_style,
                offset,
            ));
        }
        if let Some(seven_day) = context.builtin.seven_day {
            blocks.push(render_builtin_window(
                "7d",
                seven_day,
                ResetStyle::DateTime,
                context.meter_style,
                offset,
            ));
        }
    } else if let Some(oauth) = context.oauth {
        blocks.push(render_oauth_window(
            "5h",
            &oauth.five_hour,
            ResetStyle::Time,
            context.meter_style,
            offset,
        ));
        blocks.push(render_oauth_window(
            "7d",
            &oauth.seven_day,
            ResetStyle::DateTime,
            context.meter_style,
            offset,
        ));
    } else {
        blocks.push(format!("{DIM}5h: -{RESET}"));
        blocks.push(format!("{DIM}7d: -{RESET}"));
    }

    if let Some(oauth) = context.oauth {
        for weekly_scope in &oauth.weekly_scoped {
            blocks.push(render_weekly_scope(
                weekly_scope,
                context.meter_style,
                offset,
            ));
        }
        if oauth.extra_usage.is_enabled {
            blocks.push(render_extra_usage(&oauth.extra_usage));
        }
    }

    if let Some(first_block) = blocks.first_mut() {
        first_block.insert_str(0, "📊 ");
    }

    blocks.join(&format!(" {DIM}·{RESET} "))
}

#[derive(Clone, Copy)]
enum ResetStyle {
    Time,
    DateTime,
}

fn render_builtin_window<O: LocalOffset>(
    label: &str,
    limit: BuiltInLimit,
    reset_style: ResetStyle,
    meter_style: MeterStyle,
    offset: &O,
) -> String {
    let percentage = builtin_percentage(limit.used_percentage);
    let mut block = format!(
        "{DIM}{label}: {RESET}{}",
        render_meter(i64::from(percentage), meter_style)
    );

    if let Some(reset_at) = limit
        .resets_at
        .and_then(|reset_at| format_epoch_reset(reset_at, reset_style, offset))
    {
        block.push_str(&format!(" {DIM}@{reset_at}{RESET}"));
    }

    block
}

fn render_oauth_window<O: LocalOffset>(
    label: &str,
    window: &crate::oauth::UsageWindow,
    reset_style: ResetStyle,
    meter_style: MeterStyle,
    offset: &O,
) -> String {
    let percentage = i64::from(window.utilization);
    let mut block = format!(
        "{DIM}{label}: {RESET}{}",
        render_meter(percentage, meter_style)
    );

    if let Some(reset_at) = format_iso_reset(&window.resets_at, reset_style, offset) {
        block.push_str(&format!(" {DIM}@{reset_at}{RESET}"));
    }

    block
}

fn render_weekly_scope<O: LocalOffset>(
    scope: &crate::oauth::WeeklyScopedUsage,
    meter_style: MeterStyle,
    offset: &O,
) -> String {
    let percentage = i64::from(scope.percent);
    let mut block = format!(
        "{DIM}{}: {RESET}{}",
        scope.display_name,
        render_meter(percentage, meter_style)
    );

    if let Some(reset_at) = format_iso_reset(&scope.resets_at, ResetStyle::DateTime, offset) {
        block.push_str(&format!(" {DIM}@{reset_at}{RESET}"));
    }

    block
}

fn render_extra_usage(extra_usage: &crate::oauth::ExtraUsage) -> String {
    let color = bar_usage_color(i64::from(extra_usage.utilization));

    format!(
        "{DIM}extra: {RESET}{color}${:.2}/${:.2}{RESET}",
        extra_usage.used_credits, extra_usage.monthly_limit
    )
}

fn bar_usage_color(percentage: i64) -> &'static str {
    match percentage.clamp(0, 100) {
        90..=100 => RED,
        70..=89 => ORANGE,
        50..=69 => YELLOW,
        _ => GREEN,
    }
}

fn builtin_percentage(percentage: f64) -> u8 {
    if !percentage.is_finite() || percentage <= 0.0 {
        0
    } else if percentage >= 100.0 {
        100
    } else {
        percentage as u8
    }
}

fn format_epoch_reset<O: LocalOffset>(
    epoch_seconds: u64,
    style: ResetStyle,
    offset: &O,
) -> Option<String> {
    let epoch_seconds = i64::try_from(epoch_seconds).ok()?;
    let local_epoch = epoch_seconds.checked_add(i64::from(offset.offset_seconds(epoch_seconds)))?;
    let civil_time = civil_time(local_epoch);

    match style {
        ResetStyle::Time => Some(format!("{:02}:{:02}", civil_time.hour, civil_time.minute)),
        ResetStyle::DateTime => {
            let month_index = usize::try_from(civil_time.month.checked_sub(1)?).ok()?;
            let month = *MONTH_NAMES.get(month_index)?;

            Some(format!(
                "{month} {}, {:02}:{:02}",
                civil_time.day, civil_time.hour, civil_time.minute
            ))
        }
    }
}

fn format_iso_reset<O: LocalOffset>(value: &str, style: ResetStyle, offset: &O) -> Option<String> {
    let epoch_seconds = parse_iso8601_epoch(value)?;
    let local_epoch = epoch_seconds.checked_add(i64::from(offset.offset_seconds(epoch_seconds)))?;
    let civil_time = civil_time(local_epoch);

    match style {
        ResetStyle::Time => Some(format!("{:02}:{:02}", civil_time.hour, civil_time.minute)),
        ResetStyle::DateTime => {
            let month_index = usize::try_from(civil_time.month.checked_sub(1)?).ok()?;
            let month = *MONTH_NAMES.get(month_index)?;

            Some(format!(
                "{month} {}, {:02}:{:02}",
                civil_time.day, civil_time.hour, civil_time.minute
            ))
        }
    }
}

fn parse_iso8601_epoch(value: &str) -> Option<i64> {
    let (date, time_with_zone) = value.split_once('T')?;
    let (year, month, day) = parse_date(date)?;
    let (time, timezone_offset) = parse_time_and_timezone(time_with_zone)?;
    let (hour, minute, second) = parse_time(time)?;
    let days = civil_to_days(year, month, day)?;
    let local_seconds = days
        .checked_mul(86_400)?
        .checked_add(i64::from(hour) * 3_600)?
        .checked_add(i64::from(minute) * 60)?
        .checked_add(i64::from(second))?;

    local_seconds.checked_sub(i64::from(timezone_offset))
}

fn parse_date(value: &str) -> Option<(i64, u32, u32)> {
    let mut components = value.split('-');
    let year = components.next()?.parse::<i64>().ok()?;
    let month = components.next()?.parse::<u32>().ok()?;
    let day = components.next()?.parse::<u32>().ok()?;
    if components.next().is_some()
        || !(0..=9_999).contains(&year)
        || !is_valid_date(year, month, day)
    {
        return None;
    }

    Some((year, month, day))
}

fn parse_time_and_timezone(value: &str) -> Option<(&str, i32)> {
    if let Some(time) = value.strip_suffix('Z') {
        return Some((time, 0));
    }

    let (index, sign) = value
        .char_indices()
        .rev()
        .find(|(index, character)| *index >= 8 && matches!(character, '+' | '-'))?;
    let (time, timezone) = value.split_at(index);
    let timezone_offset = parse_timezone_offset(sign, timezone.get(1..)?)?;

    Some((time, timezone_offset))
}

fn parse_timezone_offset(sign: char, value: &str) -> Option<i32> {
    let (hours, minutes) = value.split_once(':')?;
    let hours = hours.parse::<i32>().ok()?;
    let minutes = minutes.parse::<i32>().ok()?;
    if hours > 23 || minutes > 59 {
        return None;
    }

    let offset = hours.checked_mul(3_600)?.checked_add(minutes * 60)?;
    match sign {
        '+' => Some(offset),
        '-' => offset.checked_neg(),
        _ => None,
    }
}

fn parse_time(value: &str) -> Option<(u32, u32, u32)> {
    let mut components = value.split(':');
    let hour = components.next()?.parse::<u32>().ok()?;
    let minute = components.next()?.parse::<u32>().ok()?;
    let seconds_with_fraction = components.next()?;
    if components.next().is_some() {
        return None;
    }
    let seconds = seconds_with_fraction
        .split(['.', ','])
        .next()?
        .parse::<u32>()
        .ok()?;
    if hour > 23 || minute > 59 || seconds > 59 {
        return None;
    }

    Some((hour, minute, seconds))
}

fn is_valid_date(year: i64, month: u32, day: u32) -> bool {
    day > 0 && days_in_month(year, month).is_some_and(|last_day| day <= last_day)
}

fn days_in_month(year: i64, month: u32) -> Option<u32> {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => Some(31),
        4 | 6 | 9 | 11 => Some(30),
        2 if is_leap_year(year) => Some(29),
        2 => Some(28),
        _ => None,
    }
}

fn is_leap_year(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn civil_to_days(year: i64, month: u32, day: u32) -> Option<i64> {
    let adjusted_year = year.checked_sub(if month <= 2 { 1 } else { 0 })?;
    let era = if adjusted_year >= 0 {
        adjusted_year
    } else {
        adjusted_year.checked_sub(399)?
    } / 400;
    let year_of_era = adjusted_year.checked_sub(era.checked_mul(400)?)?;
    let month_parameter = i64::from(month) + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * month_parameter + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;

    era.checked_mul(146_097)?
        .checked_add(day_of_era)?
        .checked_sub(719_468)
}

#[cfg(unix)]
fn system_local_offset_unix(epoch_seconds: i64) -> i32 {
    use std::mem::MaybeUninit;

    let epoch_seconds = match libc::time_t::try_from(epoch_seconds) {
        Ok(epoch_seconds) => epoch_seconds,
        Err(_) => return 0,
    };
    let mut local_time = MaybeUninit::<libc::tm>::uninit();
    let mut utc_time = MaybeUninit::<libc::tm>::uninit();

    // SAFETY: both pointers are valid writable storage and `epoch_seconds` lives
    // through these calls. A null result is handled before reading either value.
    let local_result = unsafe { libc::localtime_r(&epoch_seconds, local_time.as_mut_ptr()) };
    // SAFETY: same preconditions as `localtime_r`; its output has separate storage.
    let utc_result = unsafe { libc::gmtime_r(&epoch_seconds, utc_time.as_mut_ptr()) };
    if local_result.is_null() || utc_result.is_null() {
        return 0;
    }
    // SAFETY: the successful C functions above initialized the corresponding tm values.
    let local_time = unsafe { local_time.assume_init() };
    // SAFETY: the successful C functions above initialized the corresponding tm values.
    let utc_time = unsafe { utc_time.assume_init() };

    local_offset_from_tm(&local_time, &utc_time)
}

#[cfg(windows)]
fn system_local_offset_windows(epoch_seconds: i64) -> i32 {
    use std::mem::MaybeUninit;

    let epoch_seconds = match libc::time_t::try_from(epoch_seconds) {
        Ok(epoch_seconds) => epoch_seconds,
        Err(_) => return 0,
    };
    let mut local_time = MaybeUninit::<libc::tm>::uninit();
    let mut utc_time = MaybeUninit::<libc::tm>::uninit();

    // SAFETY: both pointers are valid writable storage and `epoch_seconds` lives
    // through these calls. A non-zero return is handled before reading either value.
    let local_result = unsafe { libc::localtime_s(local_time.as_mut_ptr(), &epoch_seconds) };
    // SAFETY: same preconditions as `localtime_s`; its output has separate storage.
    let utc_result = unsafe { libc::gmtime_s(utc_time.as_mut_ptr(), &epoch_seconds) };
    if local_result != 0 || utc_result != 0 {
        return 0;
    }
    // SAFETY: the successful C functions above initialized the corresponding tm values.
    let local_time = unsafe { local_time.assume_init() };
    // SAFETY: the successful C functions above initialized the corresponding tm values.
    let utc_time = unsafe { utc_time.assume_init() };

    local_offset_from_tm(&local_time, &utc_time)
}

#[cfg(any(unix, windows))]
fn local_offset_from_tm(local_time: &libc::tm, utc_time: &libc::tm) -> i32 {
    let local_seconds = tm_as_epoch_seconds(local_time);
    let utc_seconds = tm_as_epoch_seconds(utc_time);

    local_seconds
        .zip(utc_seconds)
        .and_then(|(local_seconds, utc_seconds)| local_seconds.checked_sub(utc_seconds))
        .and_then(|offset_seconds| i32::try_from(offset_seconds).ok())
        .unwrap_or(0)
}

#[cfg(any(unix, windows))]
fn tm_as_epoch_seconds(time: &libc::tm) -> Option<i64> {
    let year = i64::from(time.tm_year).checked_add(1_900)?;
    let month = u32::try_from(time.tm_mon.checked_add(1)?).ok()?;
    let day = u32::try_from(time.tm_mday).ok()?;
    let hour = u32::try_from(time.tm_hour).ok()?;
    let minute = u32::try_from(time.tm_min).ok()?;
    let second = u32::try_from(time.tm_sec).ok()?;
    if !is_valid_date(year, month, day) || hour > 23 || minute > 59 || second > 60 {
        return None;
    }

    civil_to_days(year, month, day)?
        .checked_mul(86_400)?
        .checked_add(i64::from(hour) * 3_600)?
        .checked_add(i64::from(minute) * 60)?
        .checked_add(i64::from(second))
}

struct CivilTime {
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
}

const MONTH_NAMES: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

fn civil_time(epoch_seconds: i64) -> CivilTime {
    let days = epoch_seconds.div_euclid(86_400);
    let seconds_of_day = epoch_seconds.rem_euclid(86_400);
    let (_, month, day) = civil_from_days(days);

    CivilTime {
        month,
        day,
        hour: (seconds_of_day / 3_600) as u32,
        minute: ((seconds_of_day % 3_600) / 60) as u32,
    }
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i64, u32, u32) {
    let days = days_since_unix_epoch + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_parameter = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_parameter + 2) / 5 + 1;
    let month = month_parameter + if month_parameter < 10 { 3 } else { -9 };

    (
        year + if month <= 2 { 1 } else { 0 },
        month as u32,
        day as u32,
    )
}

#[cfg(test)]
mod tests {
    use insta::assert_snapshot;

    use crate::oauth::{ExtraUsage, UsageResponse, UsageWindow, WeeklyScopedUsage};
    use crate::render::meter::MeterStyle;
    use crate::width::strip_ansi;

    use super::{render_limits, BuiltInLimit, BuiltInLimits, LimitRenderContext, LocalOffset};

    struct UtcOffset;

    struct FixedOffset(i32);

    impl LocalOffset for UtcOffset {
        fn offset_seconds(&self, _epoch_seconds: i64) -> i32 {
            0
        }
    }

    impl LocalOffset for FixedOffset {
        fn offset_seconds(&self, _epoch_seconds: i64) -> i32 {
            self.0
        }
    }

    fn oauth_usage() -> UsageResponse {
        UsageResponse {
            five_hour: UsageWindow {
                utilization: 20,
                resets_at: "2030-03-17T17:46:40.123+00:00".to_owned(),
            },
            seven_day: UsageWindow {
                utilization: 50,
                resets_at: "2030-03-17T17:46:40Z".to_owned(),
            },
            extra_usage: ExtraUsage {
                is_enabled: true,
                utilization: 70,
                used_credits: 12.5,
                monthly_limit: 100.0,
            },
            weekly_scoped: vec![
                WeeklyScopedUsage {
                    display_name: "Haiku".to_owned(),
                    percent: 30,
                    resets_at: "2030-03-17T17:46:40+00:00".to_owned(),
                },
                WeeklyScopedUsage {
                    display_name: "Sonnet".to_owned(),
                    percent: 40,
                    resets_at: "2030-03-17T17:46:40.999Z".to_owned(),
                },
            ],
        }
    }

    #[test]
    fn snapshots_placeholders_when_no_builtin_or_oauth_limits_exist() {
        let rendered = render_limits(
            LimitRenderContext {
                builtin: BuiltInLimits::default(),
                oauth: None,
                meter_style: MeterStyle::Bar,
            },
            &UtcOffset,
        );

        assert_snapshot!(strip_ansi(&rendered), @r###"📊 5h: - · 7d: -"###);
    }

    #[test]
    fn snapshots_builtin_windows_before_oauth() {
        let rendered = render_limits(
            LimitRenderContext {
                builtin: BuiltInLimits {
                    five_hour: Some(BuiltInLimit {
                        used_percentage: 20.0,
                        resets_at: Some(1_900_000_000),
                    }),
                    seven_day: Some(BuiltInLimit {
                        used_percentage: 50.0,
                        resets_at: Some(1_900_000_000),
                    }),
                },
                oauth: None,
                meter_style: MeterStyle::Bar,
            },
            &UtcOffset,
        );

        assert_snapshot!(
            strip_ansi(&rendered),
            @r###"📊 5h: ▓▓░░░░░░░░ 20% @17:46 · 7d: ▓▓▓▓▓░░░░░ 50% @Mar 17, 17:46"###
        );
    }

    #[test]
    fn snapshots_oauth_windows_weekly_scopes_and_extra() {
        let oauth = oauth_usage();
        let rendered = render_limits(
            LimitRenderContext {
                builtin: BuiltInLimits::default(),
                oauth: Some(&oauth),
                meter_style: MeterStyle::Bar,
            },
            &UtcOffset,
        );

        assert_snapshot!(
            strip_ansi(&rendered),
            @r###"📊 5h: ▓▓░░░░░░░░ 20% @17:46 · 7d: ▓▓▓▓▓░░░░░ 50% @Mar 17, 17:46 · Haiku: ▓▓▓░░░░░░░ 30% @Mar 17, 17:46 · Sonnet: ▓▓▓▓░░░░░░ 40% @Mar 17, 17:46 · extra: $12.50/$100.00"###
        );
    }

    #[test]
    fn snapshots_only_the_present_builtin_window_from_the_seven_day_fixture(
    ) -> Result<(), serde_json::Error> {
        let input = crate::input::parse(include_str!(
            "../../tests/fixtures/status-input-seven-day-only.json"
        ))?;
        let rendered = render_limits(
            LimitRenderContext {
                builtin: BuiltInLimits {
                    five_hour: input.five_hour_usage().map(|used_percentage| BuiltInLimit {
                        used_percentage,
                        resets_at: input.five_hour_resets_at(),
                    }),
                    seven_day: input.seven_day_usage().map(|used_percentage| BuiltInLimit {
                        used_percentage,
                        resets_at: input.seven_day_resets_at(),
                    }),
                },
                oauth: None,
                meter_style: MeterStyle::Bar,
            },
            &UtcOffset,
        );

        assert_snapshot!(
            strip_ansi(&rendered),
            @r###"📊 7d: ▓▓▓▓▓░░░░░ 50% @Mar 17, 17:46"###
        );

        Ok(())
    }

    #[test]
    fn appends_oauth_supplements_without_replacing_builtin_windows() {
        let oauth = oauth_usage();
        let rendered = render_limits(
            LimitRenderContext {
                builtin: BuiltInLimits {
                    five_hour: Some(BuiltInLimit {
                        used_percentage: 0.0,
                        resets_at: Some(1_900_000_000),
                    }),
                    seven_day: None,
                },
                oauth: Some(&oauth),
                meter_style: MeterStyle::Bar,
            },
            &UtcOffset,
        );

        assert_snapshot!(
            strip_ansi(&rendered),
            @r###"📊 5h: ░░░░░░░░░░ 0% @17:46 · Haiku: ▓▓▓░░░░░░░ 30% @Mar 17, 17:46 · Sonnet: ▓▓▓▓░░░░░░ 40% @Mar 17, 17:46 · extra: $12.50/$100.00"###
        );
        assert!(rendered.contains(super::ORANGE));
    }

    #[test]
    fn formats_utc_local_month_and_cross_year_resets() {
        let utc = UtcOffset;
        let utc_plus_one = FixedOffset(3_600);
        let new_year_eve = 1_735_687_800;

        assert_eq!(
            super::format_epoch_reset(new_year_eve, super::ResetStyle::Time, &utc),
            Some("23:30".to_owned())
        );
        assert_eq!(
            super::format_epoch_reset(new_year_eve, super::ResetStyle::DateTime, &utc),
            Some("Dec 31, 23:30".to_owned())
        );
        assert_eq!(
            super::format_epoch_reset(new_year_eve, super::ResetStyle::Time, &utc_plus_one),
            Some("00:30".to_owned())
        );
        assert_eq!(
            super::format_iso_reset(
                "2024-12-31T23:30:00.999+00:00",
                super::ResetStyle::DateTime,
                &utc_plus_one,
            ),
            Some("Jan 1, 00:30".to_owned())
        );
    }

    #[test]
    fn clamps_builtin_percentages_before_rendering() {
        assert_eq!(super::builtin_percentage(-1.0), 0);
        assert_eq!(super::builtin_percentage(100.1), 100);
    }
}
