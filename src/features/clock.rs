use std::fmt::Display;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use chrono::format::{Fixed, Item, Numeric, StrftimeItems};
use chrono::{DateTime as ChronoDateTime, Local, LocalResult, NaiveDate, TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use tokio::time::sleep;

use crate::config::ClockConfig;
use crate::FeatureTrait;
use crate::StatusBar;

pub struct Clock {
    status_bar: Arc<StatusBar>,
    config: ClockConfig,
    timezone: ClockZone,
    granularity: ClockGranularity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ClockGranularity {
    Second,
    Minute,
    Day,
}

#[derive(Clone, Copy)]
enum ClockZone {
    Local,
    Named(Tz),
}

#[async_trait::async_trait]
impl FeatureTrait for Clock {
    /// Default clock state.
    fn new(status_bar: Arc<StatusBar>) -> Self {
        Self {
            status_bar,
            config: ClockConfig::default(),
            timezone: ClockZone::Local,
            granularity: ClockGranularity::Second,
        }
    }

    /// Refresh the clock on the next visible boundary.
    async fn pull(&mut self) {
        loop {
            let (output, delay) = self._render_tick();

            *self.status_bar.clock.write().await = output;
            self.status_bar.redraw.notify_one();

            sleep(delay).await;
        }
    }
}

impl Clock {
    /// Swap feature settings.
    pub fn set_config(&mut self, config: ClockConfig) -> Result<(), String> {
        let granularity = _parse_clock_granularity(config.format.as_str())?;
        let timezone = _parse_clock_zone(config.timezone.as_str())?;

        self.config = config;
        self.timezone = timezone;
        self.granularity = granularity;

        Ok(())
    }

    /// Render the current wall clock and next wake delay.
    fn _render_tick(&self) -> (String, Duration) {
        match self.timezone {
            ClockZone::Local => self._render_local_tick(),
            ClockZone::Named(timezone) => self._render_named_tick(timezone),
        }
    }

    /// Render a tick in the machine's local timezone.
    fn _render_local_tick(&self) -> (String, Duration) {
        let now = Local::now();
        let output = self._format_output(now);
        let delay = self._next_refresh_delay(now);

        (output, delay)
    }

    /// Render a tick in an explicitly configured timezone.
    fn _render_named_tick(&self, timezone: Tz) -> (String, Duration) {
        let now = Utc::now().with_timezone(&timezone);
        let output = self._format_output(now);
        let delay = self._next_refresh_delay(now);

        (output, delay)
    }

    /// Apply the configured glyph and format string.
    fn _format_output<Tz>(&self, now: ChronoDateTime<Tz>) -> String
    where
        Tz: TimeZone,
        Tz::Offset: Display,
    {
        format!(
            "{}{}",
            self.config.glyph,
            _format_clock_value(self.config.format.as_str(), now).trim()
        )
    }

    /// Sleep only until the formatted value can change again.
    fn _next_refresh_delay<Tz>(&self, now: ChronoDateTime<Tz>) -> Duration
    where
        Tz: TimeZone,
    {
        let next_tick = match self._granularity() {
            ClockGranularity::Second => _next_second_boundary(now.clone()),
            ClockGranularity::Minute => _next_minute_boundary(now.clone()),
            ClockGranularity::Day => _next_day_boundary(now.clone()),
        };

        match next_tick.signed_duration_since(now).to_std() {
            Ok(delay) if ! delay.is_zero() => delay,
            _ => Duration::from_secs(1),
        }
    }

    /// Infer the smallest unit visible in the format string.
    fn _granularity(&self) -> ClockGranularity {
        self.granularity
    }
}

/// Format the clock string with normalized timezone labels.
fn _format_clock_value<Tz>(format: &str, now: ChronoDateTime<Tz>) -> String
where
    Tz: TimeZone,
    Tz::Offset: Display,
{
    let format = _inject_timezone_placeholder(format);
    let output = now.format(format.as_str()).to_string();
    let timezone = _timezone_label(now);

    output.replace(_timezone_placeholder(), timezone.as_str())
}

/// Replace `%Z` with a placeholder we can post-process safely.
fn _inject_timezone_placeholder(format: &str) -> String {
    let mut output = String::new();
    let mut chars = format.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '%' {
            output.push(ch);
            continue;
        }

        let next = match chars.next() {
            Some(next) => next,
            None => {
                output.push('%');
                break;
            }
        };

        if next == '%' {
            output.push('%');
            output.push('%');
            continue;
        }

        if next == 'Z' {
            output.push_str(_timezone_placeholder());
            continue;
        }

        output.push('%');
        output.push(next);
    }

    output
}

/// Return the placeholder used for `%Z` post-processing.
fn _timezone_placeholder() -> &'static str {
    "__DWM_STATUS_TZ__"
}

/// Render a friendly timezone label for the current offset.
fn _timezone_label<Tz>(now: ChronoDateTime<Tz>) -> String
where
    Tz: TimeZone,
    Tz::Offset: Display,
{
    let label = now.format("%Z").to_string();

    if label == "+00:00" || label == "+0000" {
        return "UTC".to_string();
    }

    label
}

/// Parse the configured timezone into a runtime mode.
fn _parse_clock_zone(timezone: &str) -> Result<ClockZone, String> {
    if timezone.trim().is_empty() {
        return Ok(ClockZone::Local);
    }

    match Tz::from_str(timezone.trim()) {
        Ok(timezone) => Ok(ClockZone::Named(timezone)),
        Err(_) => Err(format!("Unsupported clock timezone: {}", timezone.trim())),
    }
}

/// Validate a Chrono format and find its shortest visible unit.
fn _parse_clock_granularity(format: &str) -> Result<ClockGranularity, String> {
    // Chrono hides `%#z` and no-dot fractional seconds behind the same internal item.
    let cadence_format = format.replace("%#z", "%z");
    let items = StrftimeItems::new(cadence_format.as_str())
        .parse()
        .map_err(|err| format!("Invalid clock format {format:?}: {err}"))?;
    let mut granularity = ClockGranularity::Day;

    for item in items {
        if _item_has_second_granularity(&item) {
            return Ok(ClockGranularity::Second);
        }

        if _item_has_minute_granularity(&item) {
            granularity = ClockGranularity::Minute;
        }
    }

    Ok(granularity)
}

/// Identify parsed fields that can change within one minute.
fn _item_has_second_granularity(item: &Item<'_>) -> bool {
    matches!(
        item,
        Item::Numeric(Numeric::Second, _)
        | Item::Numeric(Numeric::Nanosecond, _)
        | Item::Numeric(Numeric::Timestamp, _)
        | Item::Fixed(Fixed::Nanosecond)
        | Item::Fixed(Fixed::Nanosecond3)
        | Item::Fixed(Fixed::Nanosecond6)
        | Item::Fixed(Fixed::Nanosecond9)
        | Item::Fixed(Fixed::RFC2822)
        | Item::Fixed(Fixed::RFC3339)
        | Item::Fixed(Fixed::Internal(_))
    )
}

/// Identify parsed fields that can change within one day.
fn _item_has_minute_granularity(item: &Item<'_>) -> bool {
    matches!(
        item,
        Item::Numeric(Numeric::Hour, _)
        | Item::Numeric(Numeric::Hour12, _)
        | Item::Numeric(Numeric::Minute, _)
        | Item::Fixed(Fixed::LowerAmPm)
        | Item::Fixed(Fixed::UpperAmPm)
        | Item::Fixed(Fixed::TimezoneName)
        | Item::Fixed(Fixed::TimezoneOffsetColon)
        | Item::Fixed(Fixed::TimezoneOffsetDoubleColon)
        | Item::Fixed(Fixed::TimezoneOffsetTripleColon)
        | Item::Fixed(Fixed::TimezoneOffsetColonZ)
        | Item::Fixed(Fixed::TimezoneOffset)
        | Item::Fixed(Fixed::TimezoneOffsetZ)
    )
}

/// Round forward to the next second boundary.
fn _next_second_boundary<Tz>(now: ChronoDateTime<Tz>) -> ChronoDateTime<Tz>
where
    Tz: TimeZone,
{
    let next = now + chrono::Duration::seconds(1);

    next.with_nanosecond(0).unwrap_or(next)
}

/// Round forward to the next minute boundary.
fn _next_minute_boundary<Tz>(now: ChronoDateTime<Tz>) -> ChronoDateTime<Tz>
where
    Tz: TimeZone,
{
    let next = now + chrono::Duration::minutes(1);
    let next = next.with_second(0).unwrap_or(next);

    next.with_nanosecond(0).unwrap_or(next)
}

/// Round forward to the next local day boundary.
fn _next_day_boundary<Tz>(now: ChronoDateTime<Tz>) -> ChronoDateTime<Tz>
where
    Tz: TimeZone,
{
    let next_day = now.date_naive() + chrono::Duration::days(1);
    let timezone = now.timezone();

    match _first_instant_of_date(&timezone, next_day) {
        Some(next) => next,
        None => _next_minute_boundary(now + chrono::Duration::days(1)),
    }
}

/// Find the earliest representable wall-clock instant on a local date.
fn _first_instant_of_date<Tz>(timezone: &Tz, date: NaiveDate) -> Option<ChronoDateTime<Tz>>
where
    Tz: TimeZone,
{
    let midnight = date.and_hms_opt(0, 0, 0)?;

    for elapsed in 0..86_400 {
        let local = midnight + chrono::Duration::seconds(elapsed);

        match timezone.from_local_datetime(&local) {
            LocalResult::Single(instant) => return Some(instant),
            LocalResult::Ambiguous(first, second) => {
                if first.timestamp() <= second.timestamp() {
                    return Some(first);
                }

                return Some(second);
            }
            LocalResult::None => {}
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::{
        _parse_clock_zone,
        _format_clock_value,
        _next_day_boundary,
        _next_minute_boundary,
        _next_second_boundary,
        Clock,
        ClockZone,
        ClockGranularity,
    };
    use crate::config::ClockConfig;
    use crate::status_bar::StatusBar;
    use crate::FeatureTrait;
    use chrono::{Datelike, FixedOffset, TimeZone, Timelike, Utc};
    use chrono_tz::{America, Tz};
    use std::sync::Arc;

    /// Recognize every parsed form whose output can change within a minute.
    #[test]
    fn second_granularity_for_second_formats() {
        for format in [
            "%S", "%s", "%T", "%f", "%.3f", "%.6f", "%.9f", "%3f", "%6f", "%9f", "%+",
        ] {
            let clock = _clock_from_format(format);

            assert_eq!(clock._granularity(), ClockGranularity::Second, "{format}");
        }
    }

    /// Refresh every minute when the format hides seconds.
    #[test]
    fn minute_granularity_for_minute_formats() {
        let clock = _clock_from_format("%H:%M");

        assert_eq!(clock._granularity(), ClockGranularity::Minute);
    }

    /// Refresh daily when the format is date-only.
    #[test]
    fn day_granularity_for_date_formats() {
        let clock = _clock_from_format("%Y-%m-%d");

        assert_eq!(clock._granularity(), ClockGranularity::Day);
    }

    /// Treat escaped percent directives as literal text.
    #[test]
    fn escaped_second_directive_has_day_granularity() {
        let clock = _clock_from_format("%%S");

        assert_eq!(clock._granularity(), ClockGranularity::Day);
    }

    /// Refresh zone-only output often enough to cross offset transitions.
    #[test]
    fn timezone_directives_have_minute_granularity() {
        for format in ["%Z", "%z", "%:z", "%::z", "%:::z", "%#z"] {
            let clock = _clock_from_format(format);

            assert_eq!(clock._granularity(), ClockGranularity::Minute, "{format}");
        }
    }

    /// Reject malformed directives while applying configuration.
    #[test]
    fn reject_invalid_clock_format() {
        let mut clock = Clock::new(Arc::new(StatusBar::new()));
        let error = clock.set_config(ClockConfig {
            glyph: String::new(),
            format: "%Q".to_string(),
            timezone: String::new(),
        }).unwrap_err();

        assert!(error.contains("Invalid clock format"));
        assert!(error.contains("%Q"));
    }

    /// Align the second refresh to the next whole second.
    #[test]
    fn next_second_boundary_zeroes_fractional_time() {
        let now = Utc.with_ymd_and_hms(2026, 3, 22, 10, 30, 45).unwrap()
            .with_nanosecond(250_000_000)
            .unwrap();
        let next = _next_second_boundary(now);

        assert_eq!(next.second(), 46);
        assert_eq!(next.nanosecond(), 0);
    }

    /// Align the minute refresh to the top of the next minute.
    #[test]
    fn next_minute_boundary_zeroes_seconds() {
        let now = Utc.with_ymd_and_hms(2026, 3, 22, 10, 30, 45).unwrap()
            .with_nanosecond(250_000_000)
            .unwrap();
        let next = _next_minute_boundary(now);

        assert_eq!(next.minute(), 31);
        assert_eq!(next.second(), 0);
        assert_eq!(next.nanosecond(), 0);
    }

    /// Align the day refresh to the next UTC midnight.
    #[test]
    fn next_day_boundary_hits_midnight() {
        let now = Utc.with_ymd_and_hms(2026, 3, 22, 10, 30, 45).unwrap();
        let next = _next_day_boundary(now);

        assert_eq!(next.day(), 23);
        assert_eq!(next.hour(), 0);
        assert_eq!(next.minute(), 0);
        assert_eq!(next.second(), 0);
    }

    /// Use the first real instant when a DST jump removes midnight.
    #[test]
    fn next_day_boundary_skips_nonexistent_midnight() {
        let timezone = America::Sao_Paulo;
        let now = timezone.with_ymd_and_hms(2018, 11, 3, 12, 0, 0).unwrap();
        let next = _next_day_boundary(now);

        assert_eq!(next.date_naive().to_string(), "2018-11-04");
        assert_eq!(next.hour(), 1);
        assert_eq!(next.minute(), 0);
    }

    /// Use local mode when the timezone string is empty.
    #[test]
    fn parse_empty_timezone_as_local() {
        let timezone = _parse_clock_zone("").unwrap();

        assert!(matches!(timezone, ClockZone::Local));
    }

    /// Reject unknown timezone names early.
    #[test]
    fn reject_invalid_timezone() {
        let timezone = _parse_clock_zone("Mars/Olympus_Mons");

        assert!(timezone.is_err());
    }

    /// Normalize zero-offset timezone labels to UTC.
    #[test]
    fn render_utc_for_zero_offset_zone_name() {
        let now = FixedOffset::east_opt(0).unwrap()
            .with_ymd_and_hms(2026, 3, 22, 10, 30, 45)
            .unwrap();
        let output = _format_clock_value("%H:%M %Z", now);

        assert_eq!(output, "10:30 UTC");
    }

    /// Render the changed zone name on both sides of a DST boundary.
    #[test]
    fn render_zone_name_across_dst_transition() {
        let timezone = America::New_York;
        let before = Utc.with_ymd_and_hms(2026, 3, 8, 6, 59, 0).unwrap()
            .with_timezone(&timezone);
        let after = Utc.with_ymd_and_hms(2026, 3, 8, 7, 0, 0).unwrap()
            .with_timezone(&timezone);

        assert_eq!(_format_clock_value("%Z", before), "EST");
        assert_eq!(_format_clock_value("%Z", after), "EDT");
    }

    /// Build a configured clock for cadence assertions.
    fn _clock_from_format(format: &str) -> Clock {
        let mut clock = Clock::new(Arc::new(StatusBar::new()));
        clock.set_config(ClockConfig {
            glyph: String::new(),
            format: format.to_string(),
            timezone: Tz::UTC.to_string(),
        }).unwrap();

        clock
    }
}
