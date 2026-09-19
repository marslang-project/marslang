# std.time

```mars
takepkg std.time;

func m{
    start = time.monotonic();
    time.sleep(0.5);
    out(time.monotonic() - start);   // about 0.5
}
```

Times are `float` seconds.

| Function | Result |
| --- | --- |
| `now()` | Seconds since 1970-01-01 UTC from the system clock; can jump if the clock is changed |
| `monotonic()` | Seconds from a fixed starting point; never goes backwards, so use it to measure elapsed time |
| `sleep(seconds)` | Pauses the program; accepts any numeric kind; negative or non-finite values raise `RangeError` |

Calendar dates and time zones are planned for `std.datetime`.
