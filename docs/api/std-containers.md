# std.containers

```mars
takepkg std.containers;

func m{
    jobs = containers.priority_queue();
    jobs.push("write docs", 2).push("fix crash", 1);
    out(jobs.pop());        // fix crash
}
```

Every container has `len()`, `is_empty()`, and `items()`, which returns a new array
of its items. `pop` and `peek` return `null` when the container is empty, like
array `pop()`. Methods that add items return the container, so calls can chain.
A `fixed` container cannot be changed.

| Constructor | Methods |
| --- | --- |
| `containers.stack()` | `push(item)`, `pop()`, `peek()`: last in, first out. `items()` is bottom to top. |
| `containers.queue()` | `push(item)`, `pop()`, `peek()`: first in, first out. `items()` is front to back. |
| `containers.deque()` | `push_front(item)`, `push_back(item)`, `pop_front()`, `pop_back()`, `peek_front()`, `peek_back()`. `items()` is front to back. |
| `containers.priority_queue()` | `push(item, priority)`, `pop()`, `peek()`: the lowest priority number comes out first; equal priorities come out in push order. Priorities are numbers of any kind. `items()` is in pop order. |

Adding or removing at either end of a deque, and at the front of a queue, does not
move the other items. The package is written in Marslang: [std/containers.mars](../../std/containers.mars).
