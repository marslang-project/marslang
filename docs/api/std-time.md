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

## Sleeping in tasks

Outside [tasks](async.md), `sleep` stops the whole program until it returns.
Inside a task it pauses only that task, as `async.sleep` does, and the other
tasks run meanwhile; it can be cancelled like any wait. Five tasks that each
call `time.sleep(0.3)` finish together after 0.3 seconds:

```mars
takepkg std.time;
takepkg std.async;

async func nap(int n){
    time.sleep(0.3);
    ret n;
}

func m{
    out(await async.all(arr(nap(1), nap(2), nap(3))));   // [1, 2, 3], after 0.3 seconds
}
```

`sleep` written like this needs no `await`, so existing code that sleeps keeps
working when it is moved into a task. A task that computes without waiting
still holds the others up.

## Lengths of time

| Function | Result |
| --- | --- |
| `days(n)` | A duration of `n` days of 24 hours each |
| `hours(n)`, `minutes(n)`, `seconds(n)`, `milliseconds(n)` | A duration in that unit |

`n` may be any numeric kind and may have a fraction: `time.hours(1.5)` is
`1h 30m`. See [dates and times](dates.md) for what durations do.
