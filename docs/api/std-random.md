# std.random

Random numbers, and random choices, shuffles, and samples: dice, cards, loot
tables, simulations, and test data.

```mars
takepkg std.random;

func m{
    out(random.int_between(1, 6));                               // a dice roll
    out(random.choice(arr("rock", "paper", "scissors")));
    deck = arr("A", "K", "Q", "J", "10");
    random.shuffle(deck);
    out(deck);
    out(random.weighted_choice(arr("common", "rare", "epic"), arr(80, 15, 5)));
}
```

Each run gives different numbers. To get the same ones every time, for a test
or a replay, set a seed first: `random.seed(2026);`.

| Function | Result |
| --- | --- |
| `float()` | A float from `0.0` up to, but never, `1.0` |
| `float_between(low, high)` | A float from `low` up to, but never, `high` |
| `int_between(low, high)` | A whole number from `low` to `high`, both included |
| `int_below(count)` | A whole number from `0` up to, but not, `count` |
| `bool()` | `true` or `false`, each half of the time |
| `chance(probability)` | `true` with that probability, from `0.0` to `1.0` |
| `choice(items)` | One item of an array |
| `choices(items, count)` | `count` items, picked one by one, so one can come up again |
| `sample(items, count)` | `count` items from different positions, in random order |
| `shuffle(items)` | Puts the array in a random order, in place; returns `null` |
| `shuffled(items)` | A shuffled copy, leaving the array alone |
| `weighted_choice(items, weights)` | One item, picked in proportion to its weight |
| `weighted_choices(items, weights, count)` | `count` items picked by weight |
| `gauss(mean, deviation)` | A float from the normal distribution (the bell curve) |
| `seed(seed)` | Starts the numbers over from a seed; returns `null` |
| `Generator(seed)` | A generator of its own, with all of the above as methods |

Every value is equally likely: `int_between(1, 6)` gives each face a sixth of
the time, and `shuffle` makes every order equally likely. `int_between` and
`int_below` give an `int` when their arguments are ints, otherwise a `longint`,
and cover the whole `longint` range.

## Choosing

`sample` deals from the array: no position is picked twice, so
`random.sample(entrants, 3)` gives three different entrants (an array that
holds the same value twice can still give it twice). `choices` draws with
replacement: `random.choices(arr("H", "T"), 10)` is ten coin flips.

Weights are numbers of `0` or more, one per item, not all `0`; they need not add
up to anything in particular. `arr(80, 15, 5)` makes `"common"` 80 times in 100.
An item of weight `0` never comes up. For many picks, `weighted_choices` adds the
weights up once rather than every time.

```mars
takepkg std.random;

func m{
    flips = random.choices(arr("H", "T"), 10);
    winners = random.sample(arr("Ada", "Grace", "Alan", "Edsger", "Barbara"), 2);
    hand = random.weighted_choices(arr("common", "rare"), arr(9, 1), 5);
    out(flips, winners, hand);
}
```

## Repeating numbers: seeds

`random.seed(value)` starts the shared generator over. After the same seed, a
program makes the same numbers on every run and on every platform: Windows,
macOS, and Linux, on x86 and ARM. The seed is an `int`, a `longint`, or a
string, such as the name of a level; `null` makes the numbers unpredictable
again.

```mars
takepkg std.random;

func m{
    random.seed("level-1");
    first = random.int_between(1, 100);
    random.seed("level-1");
    out(first == random.int_between(1, 100));   // true
}
```

The functions of `std.random` share one generator, so any part of a program
that draws numbers moves it on for every other part. `random.Generator(seed)`
makes a generator of its own, with every function above as a method, so a
procedural world stays the same however many dice the rest of the game rolls:

```mars
takepkg std.random;

func m{
    world = random.Generator(42);
    out(world.int_between(1, 6), world.shuffled(arr(1, 2, 3)));   // the same on every run
    world.seed(42);                                              // and from the start again
    noise = random.Generator(null);                              // unpredictable
    out(noise.float());
}
```

Floats from `gauss` use the platform's logarithm and cosine, which can differ
in the last digit between systems; every other function gives exactly the same
results everywhere. A future release may change which numbers a seed gives; its
release notes will say so.

## Errors

Arguments that cannot work raise `RangeError`, saying why: `int_between(6, 1)`,
`choice` from an empty array, `sample` of more items than the array has, a
probability outside `0.0` to `1.0`, a negative count, or weights that are
negative, missing, or all `0`. A seed that is not an `int`, `longint`, string,
or `null` raises `TypeError`, and so does shuffling a `fixed` array, which
cannot change.

## Not for secrets

`std.random` is not for passwords, keys, tokens, or anything an attacker must
not guess; use [std.secrets](std-secrets.md) for those: after enough of its output, the rest can be worked out. Its
generator is chosen for speed and statistical quality, as in most languages'
standard random packages.

## How it works

The generator is native ([std/rs/random.rs](../../std/rs/random.rs)), because
it needs 64-bit wrapping arithmetic that Marslang does not have, and runs for
every number. It is **xoshiro256++** (Blackman and Vigna), Julia's default
generator and the one behind Rust's `rand::rngs::SmallRng`: fast, with 256 bits
of state and a period of 2²⁵⁶ − 1, and passing the standard statistical test
suites, BigCrush and PractRand.
A seed becomes a starting state through SplitMix64, as its authors recommend;
a string seed is first hashed with FNV-1a, which never changes. Without a seed,
the state comes from the operating system's random source.

Whole numbers in a range use Lemire's method, which has no bias towards any
value and needs one multiplication per number. Everything else is Marslang, in
[std/random.mars](../../std/random.mars): Fisher–Yates shuffling, sampling (by
skipping repeats when taking few from many, and by a partial shuffle
otherwise), binary search over running totals for weights, and the Box–Muller
transform for `gauss`.
