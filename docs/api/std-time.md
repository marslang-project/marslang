# std.time

```mars
takepkg std.time;

func m{
    start = time.monotonic();
    time.sleep(time.milliseconds(500));
    out(time.monotonic() - start);   // about 0.5

    out(time.today(), time.now_datetime());
    out(time.today() + time.days(7));
}
```

## Clocks

| Function | Result |
| --- | --- |
| `now()` | Seconds since 1970-01-01 UTC from the system clock, as a float; can jump if the clock is changed |
| `monotonic()` | Seconds from a fixed starting point, as a float; never goes backwards, so use it to measure elapsed time |
| `today()` | Today's [date](dates.md), where this machine is |
| `now_datetime()` | This moment, as a [datetime](dates.md) in this machine's time zone |
| `sleep(length)` | Pauses the program for a duration or a number of seconds; negative or non-finite values raise `RangeError` |

## Lengths of time

| Function | Result |
| --- | --- |
| `days(n)` | A duration of `n` days of 24 hours each |
| `hours(n)`, `minutes(n)`, `seconds(n)`, `milliseconds(n)` | A duration in that unit |

`n` may be any numeric kind and may have a fraction: `time.hours(1.5)` is
`1h 30m`. See [dates and times](dates.md) for what durations do.
