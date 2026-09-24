//! `date`, `datetime`, and `duration`: Marslang's calendar kinds, built on
//! the jiff crate, which knows the calendar, time zones, and daylight saving.
//!
//! - A `date` is a calendar day, such as 2026-09-24, with no time or zone.
//! - A `datetime` is a moment together with the time zone it is seen in:
//!   2026-09-24T14:30:00+08:00[Asia/Shanghai]. Two datetimes are equal when
//!   they are the same moment, whatever their zones.
//! - A `duration` is an exact length of time, such as 2h 30m. A day in a
//!   duration is 24 hours; calendar steps such as months are methods,
//!   `add_months`, since a month has no fixed length.

use std::cmp::Ordering;
use std::rc::Rc;
use std::sync::OnceLock;

use jiff::civil;
use jiff::tz::{Offset, TimeZone};
use jiff::{SignedDuration, Span, Timestamp, Zoned};

use crate::value::*;

// ----- the local time zone -----

/// The time zone this machine is set to: `TZ` when it is set, then the
/// system's own setting, then UTC.
pub(crate) fn local_zone() -> TimeZone {
    static LOCAL: OnceLock<TimeZone> = OnceLock::new();
    LOCAL.get_or_init(|| {
        if let Some(name) = std::env::var_os("TZ").and_then(|v| v.into_string().ok()).filter(|v| !v.is_empty()) {
            if let Ok(zone) = TimeZone::get(name.trim_start_matches(':')) { return zone; }
        }
        system_zone().unwrap_or(TimeZone::UTC)
    }).clone()
}

#[cfg(not(windows))]
fn system_zone() -> Option<TimeZone> { Some(TimeZone::system()) }

/// Windows names its zones itself ("China Standard Time"); map that to the
/// IANA name through the CLDR table, as jiff does. Linked the ordinary way,
/// so it builds with windows-gnu toolchains that cannot link raw-dylib.
#[cfg(windows)]
fn system_zone() -> Option<TimeZone> {
    #[repr(C)]
    struct SystemTime { fields: [u16; 8] }
    #[repr(C)]
    struct DynamicTimeZoneInformation {
        bias: i32,
        standard_name: [u16; 32],
        standard_date: SystemTime,
        standard_bias: i32,
        daylight_name: [u16; 32],
        daylight_date: SystemTime,
        daylight_bias: i32,
        timezone_key_name: [u16; 128],
        dynamic_daylight_time_disabled: u8,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetDynamicTimeZoneInformation(info: *mut DynamicTimeZoneInformation) -> u32;
    }
    let mut info = std::mem::MaybeUninit::<DynamicTimeZoneInformation>::zeroed();
    // SAFETY: the structure matches DYNAMIC_TIME_ZONE_INFORMATION and the call
    // only writes into it.
    let result = unsafe { GetDynamicTimeZoneInformation(info.as_mut_ptr()) };
    if result == 0xFFFF_FFFF { return None; }
    // SAFETY: zeroed, then filled by the call; every field is plain data.
    let info = unsafe { info.assume_init() };
    let end = info.timezone_key_name.iter().position(|&c| c == 0).unwrap_or(128);
    let key = String::from_utf16(&info.timezone_key_name[..end]).ok()?;
    let (_, iana) = crate::windows_zones::WINDOWS_TO_IANA.iter().find(|(windows, _)| windows.eq_ignore_ascii_case(&key))?;
    TimeZone::get(iana).ok()
}

/// A zone from its name: "local", "UTC", an offset such as "+08:00", or an
/// IANA name such as "Asia/Shanghai".
pub(crate) fn zone(name: &str) -> RResult<TimeZone> {
    match name {
        "local" => return Ok(local_zone()),
        "UTC" | "utc" | "Z" => return Ok(TimeZone::UTC),
        _ => {}
    }
    if let Some(offset) = parse_offset(name) { return Ok(TimeZone::fixed(offset?)); }
    TimeZone::get(name).or_else(|_| range_err(format!(
        "unknown time zone \"{name}\"; use a name such as \"Asia/Shanghai\", \"UTC\", \"local\", or an offset such as \"+08:00\""
    )))
}

/// "+08:00", "-0530", or "+08": `None` when the text is not an offset at all.
fn parse_offset(text: &str) -> Option<RResult<Offset>> {
    let sign = match text.as_bytes().first()? { b'+' => 1, b'-' => -1, _ => return None };
    let digits: String = text[1..].chars().filter(|c| *c != ':').collect();
    if !(digits.len() == 2 || digits.len() == 4) || !digits.chars().all(|c| c.is_ascii_digit()) {
        return Some(range_err(format!("\"{text}\" is not an offset; write it as +08:00 or -05:30")));
    }
    let hours: i32 = digits[..2].parse().ok()?;
    let minutes: i32 = if digits.len() == 4 { digits[2..].parse().ok()? } else { 0 };
    if hours > 25 || minutes > 59 { return Some(range_err(format!("\"{text}\" is not a real offset"))); }
    Some(Offset::from_seconds(sign * (hours * 3600 + minutes * 60)).or_else(|e| range_err(e.to_string())))
}

// ----- printing -----

pub(crate) fn show_offset(offset: Offset) -> String {
    let seconds = offset.seconds();
    let sign = if seconds < 0 { '-' } else { '+' };
    let seconds = seconds.abs();
    let text = format!("{sign}{:02}:{:02}", seconds / 3600, seconds / 60 % 60);
    if seconds % 60 == 0 { text } else { format!("{text}:{:02}", seconds % 60) }
}

pub fn show_date(date: &civil::Date) -> String { date.to_string() }

/// ISO 8601 with the offset, and the zone's name in brackets when it has one
/// (RFC 9557): 2026-09-24T14:30:00+08:00[Asia/Shanghai].
pub fn show_datetime(moment: &Zoned) -> String {
    let mut text = format!("{}{}", moment.datetime(), show_offset(moment.offset()));
    if let Some(name) = moment.time_zone().iana_name() { text.push_str(&format!("[{name}]")); }
    text
}

/// Days, hours, minutes, and seconds, largest first: 1d 2h 30m 4.5s.
pub fn show_duration(length: &SignedDuration) -> String {
    if length.is_zero() { return "0s".into(); }
    let negative = length.is_negative();
    let length = length.unsigned_abs();
    let total = length.as_secs();
    let (days, hours, minutes, seconds) = (total / 86_400, total / 3600 % 24, total / 60 % 60, total % 60);
    let nanos = length.subsec_nanos();
    let mut parts = Vec::new();
    if days > 0 { parts.push(format!("{days}d")); }
    if hours > 0 { parts.push(format!("{hours}h")); }
    if minutes > 0 { parts.push(format!("{minutes}m")); }
    if seconds > 0 || nanos > 0 {
        let fraction = if nanos == 0 { String::new() } else { format!(".{nanos:09}").trim_end_matches('0').to_string() };
        parts.push(format!("{seconds}{fraction}s"));
    }
    format!("{}{}", if negative { "-" } else { "" }, parts.join(" "))
}

/// ISO 8601, which JSON readers elsewhere understand: PT2H30M.
pub fn iso_duration(length: &SignedDuration) -> String { length.to_string() }

// ----- building values -----

fn whole(value: &Value, what: &str) -> RResult<i64> {
    match value {
        Value::Int(n) | Value::Long(n) => Ok(*n),
        Value::Float(f) if f.fract() == 0.0 && f.is_finite() => Ok(*f as i64),
        other => type_err(format!("{what} must be a whole number, not {} {}", article(other.type_name()), other.type_name())),
    }
}

fn text<'a>(value: &'a Value, what: &str) -> RResult<&'a str> {
    match value {
        Value::Str(text) => Ok(text),
        other => type_err(format!("{what} must be a string, not {} {}", article(other.type_name()), other.type_name())),
    }
}

fn article(kind: &str) -> &'static str {
    if kind.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" }
}

fn number(value: &Value, what: &str) -> RResult<f64> {
    match value {
        Value::Int(n) | Value::Long(n) => Ok(*n as f64),
        Value::Float(f) if f.is_finite() => Ok(*f),
        Value::Float(_) => range_err(format!("{what} must be finite")),
        other => type_err(format!("{what} must be a number, not {} {}", article(other.type_name()), other.type_name())),
    }
}

pub fn date_value(date: civil::Date) -> Value { Value::Date(date) }
pub fn datetime_value(moment: Zoned) -> Value { Value::DateTime(Rc::new(moment)) }
pub fn duration_value(length: SignedDuration) -> Value { Value::Duration(length) }

fn civil_date(year: i64, month: i64, day: i64) -> RResult<civil::Date> {
    let fits = |n: i64, low: i64, high: i64| (low..=high).contains(&n);
    if !fits(year, -9999, 9999) { return range_err(format!("the year {year} is outside -9999 to 9999")); }
    if !fits(month, 1, 12) { return range_err(format!("the month {month} is not from 1 to 12")); }
    civil::Date::new(year as i16, month as i8, day.clamp(-1, 99) as i8).or_else(|_| {
        let days = civil::Date::new(year as i16, month as i8, 1).map(|d| d.days_in_month()).unwrap_or(31);
        range_err(format!("{year}-{month:02} has days 1 to {days}, not {day}"))
    })
}

/// `date(2026, 9, 24)`, `date("2026-09-24")`, `date("24/09/2026", "%d/%m/%Y")`,
/// or `date(a_datetime)`, its day in its own zone.
pub fn make_date(args: &[Value]) -> RResult<Value> {
    match args {
        [y, m, d] => Ok(date_value(civil_date(whole(y, "the year")?, whole(m, "the month")?, whole(d, "the day")?)?)),
        [Value::Str(text)] => text.trim().parse::<civil::Date>().map(date_value).or_else(|_| range_err(format!(
            "\"{text}\" is not a date; write it as 2026-09-24, or pass a format such as date(text, \"%d/%m/%Y\")"
        ))),
        [Value::DateTime(moment)] => Ok(date_value(moment.date())),
        [Value::Date(date)] => Ok(date_value(*date)),
        [Value::Str(input), Value::Str(format)] => civil::Date::strptime(format.as_bytes(), input.as_bytes()).map(date_value)
            .or_else(|e| range_err(format!("\"{input}\" does not match the format \"{format}\": {e}"))),
        _ => type_err("date takes a year, month, and day; a string such as \"2026-09-24\"; a string and its format; or a datetime"),
    }
}

/// `datetime("2026-09-24T14:30:00+08:00")`, with a zone in brackets or not;
/// `datetime(text, format)`; or `datetime(year, month, day, hour, minute,
/// second, zone)`.
pub fn make_datetime(args: &[Value]) -> RResult<Value> {
    match args {
        [Value::Str(input)] => parse_datetime(input.trim()).map(datetime_value),
        [Value::Str(input), Value::Str(format)] => {
            let broken = jiff::fmt::strtime::parse(format.as_bytes(), input.as_bytes())
                .or_else(|e| range_err(format!("\"{input}\" does not match the format \"{format}\": {e}")))?;
            broken.to_zoned().map(datetime_value).or_else(|e| range_err(format!(
                "\"{input}\" read with \"{format}\" has no time zone; add %z (an offset) or %Q (a zone name) to the format: {e}"
            )))
        }
        [y, mo, d, h, mi, s, Value::Str(name)] => {
            let date = civil_date(whole(y, "the year")?, whole(mo, "the month")?, whole(d, "the day")?)?;
            let (h, mi, s) = (whole(h, "the hour")?, whole(mi, "the minute")?, whole(s, "the second")?);
            if !(0..24).contains(&h) || !(0..60).contains(&mi) || !(0..60).contains(&s) {
                return range_err(format!("{h:02}:{mi:02}:{s:02} is not a time of day"));
            }
            let time = civil::Time::new(h as i8, mi as i8, s as i8, 0).expect("checked above");
            // A time skipped by a daylight-saving change moves forward; one that
            // happens twice takes the first, as most calendars do.
            date.to_datetime(time).to_zoned(zone(name)?).map(datetime_value).or_else(|e| range_err(e.to_string()))
        }
        _ => type_err("datetime takes a string such as \"2026-09-24T14:30:00+08:00\"; a string and its format; or year, month, day, hour, minute, second, and a zone"),
    }
}

fn parse_datetime(input: &str) -> RResult<Zoned> {
    use jiff::fmt::temporal::{Pieces, PiecesOffset};
    let problem = || range_err(format!(
        "\"{input}\" is not a datetime; write it as 2026-09-24T14:30:00+08:00, optionally followed by a zone such as [Asia/Shanghai]"
    ));
    let Ok(pieces) = Pieces::parse(input) else { return problem() };
    if pieces.time_zone_annotation().is_some() {
        return input.parse::<Zoned>().or_else(|e| range_err(format!("\"{input}\": {e}")));
    }
    let time = pieces.time().unwrap_or(civil::Time::midnight());
    let at = pieces.date().to_datetime(time);
    let zone = match pieces.offset() {
        Some(PiecesOffset::Zulu) => TimeZone::UTC,
        Some(PiecesOffset::Numeric(offset)) => TimeZone::fixed(offset.offset()),
        _ => return range_err(format!(
            "\"{input}\" does not say which time zone it is in; add an offset such as +08:00 or Z, or a zone such as [Asia/Shanghai]"
        )),
    };
    at.to_zoned(zone).or_else(|e| range_err(e.to_string()))
}

/// `duration(90)` seconds, `duration(1.5)`, `duration("2h 30m")`, or ISO 8601
/// `duration("PT2H30M")`.
pub fn make_duration(args: &[Value]) -> RResult<Value> {
    match args {
        [Value::Str(input)] => parse_duration(input.trim()).map(duration_value),
        [Value::Duration(length)] => Ok(duration_value(*length)),
        [value] => seconds(number(value, "a duration in seconds")?).map(duration_value),
        _ => type_err("duration takes a number of seconds or a string such as \"2h 30m\""),
    }
}

pub fn seconds(seconds: f64) -> RResult<SignedDuration> {
    SignedDuration::try_from_secs_f64(seconds).or_else(|_| range_err(format!("{seconds} seconds is too long for a duration")))
}

/// "1d 2h 30m 4.5s" as `show_duration` writes it, or ISO 8601.
fn parse_duration(input: &str) -> RResult<SignedDuration> {
    let problem = || range_err(format!("\"{input}\" is not a duration; write it as 2h 30m, 1d 4h, 90s, or PT2H30M"));
    if input.starts_with(['P', 'p']) || input.starts_with(['-', '+']) && input[1..].starts_with(['P', 'p']) {
        return input.parse::<SignedDuration>().or_else(|_| problem());
    }
    let (negative, body) = match input.strip_prefix('-') { Some(rest) => (true, rest.trim_start()), None => (false, input) };
    let mut total = 0.0;
    let mut any = false;
    let mut rest = body;
    while !rest.is_empty() {
        let digits = rest.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(rest.len());
        if digits == 0 { return problem(); }
        let Ok(amount) = rest[..digits].parse::<f64>() else { return problem() };
        // "2h" and "2 hours" alike.
        let after = rest[digits..].trim_start();
        let unit = after.find(|c: char| !c.is_ascii_alphabetic()).unwrap_or(after.len());
        let scale = match &after[..unit] {
            "d" | "day" | "days" => 86_400.0,
            "h" | "hr" | "hour" | "hours" => 3600.0,
            "m" | "min" | "minute" | "minutes" => 60.0,
            "s" | "sec" | "second" | "seconds" => 1.0,
            "ms" => 0.001,
            _ => return problem(),
        };
        total += amount * scale;
        any = true;
        rest = after[unit..].trim_start_matches([' ', ',']);
    }
    if !any { return problem(); }
    seconds(if negative { -total } else { total })
}

// ----- comparing -----

/// Order within one kind; `None` for values of different kinds.
pub fn order(a: &Value, b: &Value) -> Option<Ordering> {
    match (a, b) {
        (Value::Date(x), Value::Date(y)) => Some(x.cmp(y)),
        (Value::DateTime(x), Value::DateTime(y)) => Some(x.timestamp().cmp(&y.timestamp())),
        (Value::Duration(x), Value::Duration(y)) => Some(x.cmp(y)),
        _ => None,
    }
}

pub fn is_calendar(value: &Value) -> bool {
    matches!(value, Value::Date(_) | Value::DateTime(_) | Value::Duration(_))
}

// ----- arithmetic -----

fn too_far<T>() -> RResult<T> { range_err("the result is outside the years -9999 to 9999") }

fn whole_days(length: &SignedDuration) -> RResult<i32> {
    if length.subsec_nanos() != 0 || length.as_secs() % 86_400 != 0 {
        return range_err(format!(
            "a date moves by whole days, and {} is not; use a datetime for hours and minutes", show_duration(length)
        ));
    }
    i32::try_from(length.as_secs() / 86_400).or_else(|_| too_far())
}

/// `+`, `-`, `*`, and `/` when a calendar kind is involved. `None` when
/// neither side is one, so ordinary arithmetic applies.
pub fn arithmetic(op: &str, a: &Value, b: &Value) -> Option<RResult<Value>> {
    if !is_calendar(a) && !is_calendar(b) { return None; }
    Some(calculate(op, a, b))
}

fn calculate(op: &str, a: &Value, b: &Value) -> RResult<Value> {
    use Value::{Date as D, DateTime as T, Duration as L};
    match (a, op, b) {
        (L(x), "+", L(y)) => x.checked_add(*y).map(duration_value).ok_or(()).or_else(|_| range_err("the duration is too long")),
        (L(x), "-", L(y)) => x.checked_sub(*y).map(duration_value).ok_or(()).or_else(|_| range_err("the duration is too long")),
        (L(x), "*", n) | (n, "*", L(x)) if n.num_kind().is_some() => {
            let factor = number(n, "a duration's multiplier")?;
            seconds(x.as_secs_f64() * factor).map(duration_value)
        }
        (L(x), "/", L(y)) => {
            if y.is_zero() { return range_err("division by a zero duration"); }
            Ok(Value::Float(x.as_secs_f64() / y.as_secs_f64()))
        }
        (L(x), "/", n) if n.num_kind().is_some() => {
            let divisor = number(n, "a duration's divisor")?;
            if divisor == 0.0 { return range_err("division by zero"); }
            seconds(x.as_secs_f64() / divisor).map(duration_value)
        }
        (D(x), "+" | "-", L(y)) => {
            let days = whole_days(y)?;
            let days = if op == "-" { -days } else { days };
            x.checked_add(Span::new().days(days)).map(date_value).or_else(|_| too_far())
        }
        (L(y), "+", D(x)) => calculate("+", &D(*x), &L(*y)),
        (D(x), "-", D(y)) => Ok(duration_value(y.duration_until(*x))),
        (T(x), "+" | "-", L(y)) => {
            let step = if op == "-" { y.checked_neg().ok_or(()).or_else(|_| too_far())? } else { *y };
            x.checked_add(step).map(datetime_value).or_else(|_| too_far())
        }
        (L(y), "+", T(x)) => calculate("+", &T(x.clone()), &L(*y)),
        (T(x), "-", T(y)) => Ok(duration_value(y.duration_until(x))),
        _ => {
            let (left, right) = (a.type_name(), b.type_name());
            let hint = match (a, b) {
                (D(_), T(_)) | (T(_), D(_)) => "; convert with date(a_datetime) or a_date.at(hour, minute, second, zone)",
                (D(_) | T(_), _) | (_, D(_) | T(_)) if b.num_kind().is_some() || a.num_kind().is_some() => {
                    "; add a duration, such as time.days(3) or time.hours(2)"
                }
                _ => "",
            };
            type_err(format!("cannot {} {} {} and {} {}{hint}", verb(op), article(left), left, article(right), right))
        }
    }
}

fn verb(op: &str) -> &'static str {
    match op { "+" => "add", "-" => "subtract", "*" => "multiply", "/" => "divide", _ => "combine" }
}

pub fn negate(value: &Value) -> Option<RResult<Value>> {
    match value {
        Value::Duration(x) => Some(x.checked_neg().map(duration_value).ok_or(()).or_else(|_| range_err("the duration is too long"))),
        Value::Date(_) | Value::DateTime(_) => Some(type_err(format!("a {} cannot be negated", value.type_name()))),
        _ => None,
    }
}

// ----- methods -----

fn arity(kind: &str, name: &str, args: &[Value], n: usize) -> RResult<()> {
    if args.len() == n { return Ok(()); }
    type_err(format!("{kind}.{name}() takes {n} argument{}, got {}", if n == 1 { "" } else { "s" }, args.len()))
}

fn format(pattern: &Value, value: impl Into<jiff::fmt::strtime::BrokenDownTime>) -> RResult<Value> {
    let pattern = text(pattern, "a format")?;
    jiff::fmt::strtime::format(pattern.as_bytes(), value).map(|t| Value::str(&t))
        .or_else(|e| range_err(format!("cannot format with \"{pattern}\": {e}")))
}

fn int(n: impl Into<i64>) -> Value { Value::Int(n.into()) }

/// A calendar step: `add_days` keeps the time of day across a daylight-saving
/// change, where adding `time.days(1)` adds exactly 24 hours.
fn days(kind: &str, name: &str, args: &[Value]) -> RResult<Span> {
    arity(kind, name, args, 1)?;
    let count = whole(&args[0], "the number of days")?;
    if count.abs() > 7_304_484 { return too_far(); }
    Ok(Span::new().days(count))
}

fn months(kind: &str, name: &str, args: &[Value], per: i64) -> RResult<Span> {
    arity(kind, name, args, 1)?;
    let count = whole(&args[0], "the number")?.checked_mul(per).filter(|n| n.abs() <= 239_976);
    let Some(count) = count else { return too_far() };
    Ok(Span::new().months(count))
}

pub fn date_method(date: &civil::Date, name: &str, args: &[Value]) -> RResult<Value> {
    let none = |n: &str| arity("date", n, args, 0);
    match name {
        "year" => { none(name)?; Ok(int(date.year())) }
        "month" => { none(name)?; Ok(int(date.month())) }
        "day" => { none(name)?; Ok(int(date.day())) }
        "weekday" => { none(name)?; Ok(int(date.weekday().to_monday_one_offset())) }
        "day_of_year" => { none(name)?; Ok(int(date.day_of_year())) }
        "days_in_month" => { none(name)?; Ok(int(date.days_in_month())) }
        "is_leap_year" => { none(name)?; Ok(Value::Bool(date.in_leap_year())) }
        "add_days" => date.checked_add(days("date", name, args)?).map(date_value).or_else(|_| too_far()),
        "add_months" => date.checked_add(months("date", name, args, 1)?).map(date_value).or_else(|_| too_far()),
        "add_years" => date.checked_add(months("date", name, args, 12)?).map(date_value).or_else(|_| too_far()),
        "at" => {
            arity("date", name, args, 4)?;
            let mut parts = vec![int(date.year()), int(date.month()), int(date.day())];
            parts.extend_from_slice(args);
            make_datetime(&parts)
        }
        "format" => { arity("date", name, args, 1)?; format(&args[0], *date) }
        _ => type_err(format!("date has no method '{name}'")),
    }
}

pub fn datetime_method(moment: &Zoned, name: &str, args: &[Value]) -> RResult<Value> {
    let none = |n: &str| arity("datetime", n, args, 0);
    match name {
        "date" => { none(name)?; Ok(date_value(moment.date())) }
        "year" => { none(name)?; Ok(int(moment.year())) }
        "month" => { none(name)?; Ok(int(moment.month())) }
        "day" => { none(name)?; Ok(int(moment.day())) }
        "hour" => { none(name)?; Ok(int(moment.hour())) }
        "minute" => { none(name)?; Ok(int(moment.minute())) }
        "second" => { none(name)?; Ok(int(moment.second())) }
        "weekday" => { none(name)?; Ok(int(moment.weekday().to_monday_one_offset())) }
        "zone" => {
            none(name)?;
            Ok(Value::str(&moment.time_zone().iana_name().map(String::from).unwrap_or_else(|| show_offset(moment.offset()))))
        }
        "offset" => { none(name)?; Ok(duration_value(SignedDuration::from_secs(moment.offset().seconds() as i64))) }
        "in_zone" => {
            arity("datetime", name, args, 1)?;
            Ok(datetime_value(moment.with_time_zone(zone(text(&args[0], "a zone")?)?)))
        }
        "timestamp" => { none(name)?; Ok(Value::Float(moment.timestamp().as_duration().as_secs_f64())) }
        "add_days" => moment.checked_add(days("datetime", name, args)?).map(datetime_value).or_else(|_| too_far()),
        "add_months" => moment.checked_add(months("datetime", name, args, 1)?).map(datetime_value).or_else(|_| too_far()),
        "add_years" => moment.checked_add(months("datetime", name, args, 12)?).map(datetime_value).or_else(|_| too_far()),
        "format" => { arity("datetime", name, args, 1)?; format(&args[0], moment) }
        _ => type_err(format!("datetime has no method '{name}'")),
    }
}

pub fn duration_method(length: &SignedDuration, name: &str, args: &[Value]) -> RResult<Value> {
    arity("duration", name, args, 0)?;
    let seconds = length.as_secs_f64();
    match name {
        "seconds" => Ok(Value::Float(seconds)),
        "minutes" => Ok(Value::Float(seconds / 60.0)),
        "hours" => Ok(Value::Float(seconds / 3600.0)),
        "days" => Ok(Value::Float(seconds / 86_400.0)),
        "abs" => Ok(duration_value(SignedDuration::try_from(length.unsigned_abs()).unwrap_or(SignedDuration::MAX))),
        _ => type_err(format!("duration has no method '{name}'")),
    }
}

// ----- clocks, for std.time -----

pub fn today() -> Value { date_value(Timestamp::now().to_zoned(local_zone()).date()) }

pub fn now() -> Value { datetime_value(Timestamp::now().to_zoned(local_zone())) }
