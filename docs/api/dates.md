# Dates and times

Three built-in kinds handle the calendar and the clock:

| Kind | Is | Prints as |
| --- | --- | --- |
| `date` | A calendar day, with no time or zone | `2026-09-24` |
| `datetime` | A moment, seen in a time zone | `2026-09-24T14:30:00+08:00[Asia/Shanghai]` |
| `duration` | An exact length of time | `2h 30m` |

```mars
takepkg std.time;

func m{
    deadline = date(2026, 12, 31);
    left = deadline - time.today();
    out("days left:", left.days());

    meeting = datetime(2026, 9, 24, 14, 30, 0, "Asia/Shanghai");
    out(meeting.in_zone("America/New_York"));   // 2026-09-24T02:30:00-04:00[America/New_York]
    out(meeting + time.hours(1) + time.minutes(30));
}
```

They are values, like numbers and strings, not families: two dates of the same
day are equal, `<` and sorting order them, and they work as set members and map
keys. Annotations use their names: `func remind(date day)`. The calendar,
leap years, time zones, and daylight saving come from the
[jiff](https://crates.io/crates/jiff) crate.

## date

| Written | Gives |
| --- | --- |
| `date(2026, 9, 24)` | Year, month, and day |
| `date("2026-09-24")` | ISO 8601 text |
| `date("24/09/2026", "%d/%m/%Y")` | Text in another layout; see [formats](#formats) |
| `date(a_datetime)` | The day of a datetime, in its own zone |
| `time.today()` | Today, where this machine is |

| Method | Result |
| --- | --- |
| `year()`, `month()`, `day()` | The parts, as ints |
| `weekday()` | 1 for Monday to 7 for Sunday |
| `day_of_year()` | 1 to 366 |
| `days_in_month()` | 28 to 31 |
| `is_leap_year()` | Whether its year has a February 29 |
| `add_days(n)`, `add_months(n)`, `add_years(n)` | Another date, `n` steps away; `n` may be negative |
| `at(hour, minute, second, zone)` | A datetime on this day |
| `format(pattern)` | Text in any layout |

`add_months` keeps the day where it can and uses the month's last day where it
cannot: January 31 plus a month is February 28, or 29 in a leap year.

## datetime

| Written | Gives |
| --- | --- |
| `datetime("2026-09-24T14:30:00+08:00")` | ISO 8601 with an offset, or `Z` for UTC |
| `datetime("2026-09-24T14:30:00+08:00[Asia/Shanghai]")` | The same, in a named zone |
| `datetime(2026, 9, 24, 14, 30, 0, "Asia/Shanghai")` | Its parts and a zone |
| `datetime("2026-09-24 14:30 +0800", "%Y-%m-%d %H:%M %z")` | Text in another layout, which must include the zone |
| `time.now_datetime()` | Now, in this machine's zone |

A zone is an IANA name such as `"Asia/Shanghai"` or `"Europe/London"`, `"UTC"`,
an offset such as `"+08:00"`, or `"local"` for this machine's zone. A time
without a zone names no moment, so `datetime("2026-09-24T14:30:00")` raises and
asks for one.

| Method | Result |
| --- | --- |
| `year()`, `month()`, `day()`, `hour()`, `minute()`, `second()` | The parts, in its zone |
| `weekday()` | 1 for Monday to 7 for Sunday |
| `date()` | Its calendar day |
| `zone()` | The zone's name, or its offset when it has no name |
| `offset()` | Its distance from UTC at that moment, as a duration |
| `in_zone(zone)` | The same moment, seen in another zone |
| `timestamp()` | Seconds since 1970-01-01 UTC, as a float |
| `add_days(n)`, `add_months(n)`, `add_years(n)` | Calendar steps that keep the time of day |
| `format(pattern)` | Text in any layout |

Two datetimes are equal when they are the same moment, whatever zones they are
seen in: 14:30 in Shanghai equals 06:30 UTC.

### Daylight saving

Adding a duration adds exactly that much time. Calendar steps keep the time of
day. Across the night the clocks change, they differ:

```mars
ny = datetime(2026, 3, 7, 12, 0, 0, "America/New_York");
out(ny + time.days(1));   // 2026-03-08T13:00:00-04:00[America/New_York]: exactly 24 hours later
out(ny.add_days(1));      // 2026-03-08T12:00:00-04:00[America/New_York]: noon the next day
```

A time the clocks skip moves forward past the gap; a time that happens twice
takes the first.

## duration

| Written | Gives |
| --- | --- |
| `time.days(n)`, `time.hours(n)`, `time.minutes(n)`, `time.seconds(n)`, `time.milliseconds(n)` | A length in one unit; `n` may have a fraction |
| `duration(90)` | A number of seconds |
| `duration("2h 30m")`, `duration("2 hours, 30 minutes")` | Text in `d`, `h`, `m`, `s`, and `ms` |
| `duration("PT2H30M")` | ISO 8601 |
| `later - earlier` | The time between two dates or two datetimes |

| Method | Result |
| --- | --- |
| `seconds()`, `minutes()`, `hours()`, `days()` | The whole length in that unit, as a float |
| `abs()` | The length without its sign |

A day in a duration is always 24 hours. Months and years have no fixed length,
so they are the `add_months` and `add_years` methods instead. A zero duration
is false in a condition, as `0` is.

## Arithmetic

| Written | Gives |
| --- | --- |
| `date + duration`, `date - duration` | A date; the duration must be whole days |
| `date - date` | A duration |
| `datetime + duration`, `datetime - duration` | A datetime |
| `datetime - datetime` | A duration |
| `duration + duration`, `duration - duration`, `-duration` | A duration |
| `duration * number`, `duration / number` | A duration |
| `duration / duration` | How many times one fits in the other, as a float |

Mixing a date and a datetime is an error that says how to convert one, and so
is adding a number to a date, since `3` could mean days, hours, or seconds:

```text
TypeError: cannot add a date and an int; add a duration, such as time.days(3) or time.hours(2)
```

## Formats

`format` and the two-argument `date` and `datetime` use strftime patterns:

| Pattern | Gives |
| --- | --- |
| `%Y`, `%m`, `%d` | 2026, 09, 24 |
| `%B`, `%b` | September, Sep |
| `%A`, `%a` | Thursday, Thu |
| `%H`, `%M`, `%S` | 14, 30, 00 |
| `%I`, `%p` | 02, PM |
| `%z`, `%Z`, `%Q` | +0800, CST, Asia/Shanghai |
| `%j` | 267, the day of the year |
| `%%` | A `%` |

```mars
out(date(2026, 9, 24).format("%A, %d %B %Y"));   // Thursday, 24 September 2026
```

## In files

JSON has no dates, so `json.stringify` writes them as ISO 8601 strings, which
`date()`, `datetime()`, and `duration()` read back. TOML has dates:
[std.file.toml](std-file-toml.md) reads them as `date` and `datetime` and
writes them back as TOML dates. CSV, `.env`, and INI write them as they print.

## The local zone

`time.today()`, `time.now_datetime()`, and the `"local"` zone use the `TZ`
environment variable when it names a zone, and otherwise the zone the system is
set to. Where neither is known, they use UTC.

Zone names such as `"Asia/Shanghai"` come from the system's time zone database
on Linux and macOS, which the system keeps up to date. Windows has no such
database, and minimal Linux containers often leave it out, so a copy is built
into `marslang` and used when the system has none.
