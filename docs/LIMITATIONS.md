# Marslang: missing features and design choices

Updated 2026-09-16. Start with **section 1** for the highest-value additions.

This is a design inventory, not a new specification or a promise to implement
everything. Entries marked **Approved; pending** reflect the user's September 16
decisions; other comparison labels are not accepted Marslang keywords. Keep the syntax established by
[Instr.txt](../Instr.txt) and [LANGUAGE_CORE.md](LANGUAGE_CORE.md).
“Common” here means familiar across mainstream languages or their libraries,
not a measured popularity ranking. No language needs every feature below.

## Status key

| Status | Meaning |
| --- | --- |
| Missing | No defined feature/API in the current language contract |
| Undecided | Related behavior exists, but its rules are incomplete |
| Approved; pending | Requested addition; detailed design and compiler/library implementation remain pending |
| Implementation gap | Already requested; the reviewed Rust compiler does not fully deliver it |
| Excluded | Explicitly outside the original request |

The inventory is based on the original prompts, language contract, and
[compiler review](PROJECT_REVIEW.md). Accidental acceptance through raw JavaScript
emission does not establish language support. This is not a fresh execution audit
of every entry.

## 1. Recommended priorities

These priorities are project recommendations, not approved language changes.

1. **Settle scope, assignment, and value behavior.** Decide declaration versus
   reassignment, shadowing, equality, numeric overflow, and mutation through aliases.
   These decisions affect almost everything else.
2. **Add maps and basic collection inspection.** A compiler needs name-to-symbol
   lookup, lengths, membership checks, and predictable iteration.
3. **Complete loop control and reusable data models.** Implement the approved while,
   break/continue, and iterable-for forms; consider type aliases and named AST/token variants.
4. **Define modules and host services.** Exports, files, paths, arguments, and clear
   errors make real programs and self-hosting practical.
5. **Add convenience and concurrency later.** Stack/queue libraries, richer function
   calls, generators, and asynchronous I/O can follow the stable core.

## 2. Data structures

### Everyday containers

| Feature | What it is useful for | Marslang status and possible approach |
| --- | --- | --- |
| Dictionary / map | Look up a value by a key: names to symbols, configuration, counts | **Approved; pending builtin.** Decide one type versus two, syntax, key rules, missing keys, and iteration order |
| Stack | Last item added is first removed; undo history, parser work lists | **Approved; pending** `std.containers` facility exposed as `containers.stack`; API pending |
| Queue | First item added is first removed; pending jobs, breadth-first traversal | **Approved; pending** `containers.queue`; use deliberate queue storage for efficiency |
| Deque | Add/remove efficiently at either end; sliding windows and queues | **Approved; pending builtin.** Constructor/type spelling and methods remain open |
| Priority queue / heap | Retrieve the next item by priority; scheduling and graph algorithms | **Approved; pending** `containers.priority_queue`; comparison and tie rules remain open |

C++ provides `std::stack` and `std::queue` as standard-library container adaptors,
not language keywords. Marslang can take the same general approach without copying
C++ method names. See Microsoft's [stack documentation](https://learn.microsoft.com/en-us/cpp/standard-library/stack-class?view=msvc-170)
and [queue documentation](https://learn.microsoft.com/en-us/cpp/standard-library/queue-class?view=msvc-170).
Python's [deque](https://docs.python.org/3/library/collections.html#collections.deque)
is another example of a library supplying efficient operations at both ends.

### Additional containers and common operations

| Feature | What it adds | Marslang status and design note |
| --- | --- | --- |
| Immutable tuple / record | Group values that should not change, including more than two fields | **Missing as a dedicated value type.** The existing pair is mutable; families already provide some record-like structure |
| Ordered map/set, multiset | Sorted traversal or counts of duplicate values | **Missing as distinct APIs.** Set order is undecided; choose only if real programs need these |
| Byte buffer / byte type | Binary files, network protocols, encodings | **Missing.** Strings and `char` are not a complete binary-data contract |
| Length, emptiness, membership, iteration | Inspect and traverse containers uniformly | **Missing as a general protocol.** Arrays have lookup methods, but no complete shared inspection API |
| Copying, equality, hashing | Compare contents; copy without unexpected shared mutation; use objects as keys | **Copying approved; pending:** assignment is shallow and preserves fixed status; `.copy()` is deep and does not inherit fixed status. Equality/hash and graph/resource-copy details remain undecided |

Linked lists, trees, graphs, and specialized matrices are possible library types;
they need not all become builtins. Array-backed emulation of an operation does not
promise the performance of a specialized container.

## 3. Names and assignment (`:=` excluded)

| Feature | Purpose | Marslang status |
| --- | --- | --- |
| Explicit declaration versus update | Make it clear whether a new binding is created | **Undecided.** Bare `=` exists, but repeated and nested-scope assignment rules need a decision |
| `global` / outer-scope declaration | Allow a function to rebind a name outside its local scope | **Missing keyword; undecided semantics.** Top-level variables already exist; that is different from a rule for rebinding them |
| Closures and captured variables | A returned or nested function remembers surrounding variables | **Missing as a defined contract.** Decide capture, mutation, and lifetime rules |
| Destructuring / multiple assignment | Extract several fields or returned values in one statement | **Missing generally.** The requested error-array unpacking is a narrower feature |
| Assignment expression | Assign a result while using it in a larger expression | **`:=` excluded by user decision.** Do not add it as either an expression or declaration operator |

`:=` is no longer a candidate. Resolve declaration/reassignment within the
remaining Marslang syntax. The constant modifier remains **`fixed`**; the user
corrected the temporary Java-inspired spelling `final`. See the contract for
fixed propagation and `.copy()` examples using `name (type)` declarations.

Python's [`global` and `nonlocal`](https://docs.python.org/3/reference/simple_stmts.html#the-global-statement)
distinguish module-level rebinding from enclosing-function rebinding. Marslang
could use keywords, explicit declarations, or another scope rule. It does not
need a `global` keyword merely to support top-level values.

## 4. Control flow and expressions

| Feature | Purpose | Marslang status and caveat |
| --- | --- | --- |
| While / do-while / infinite loop | Repeat until a condition changes, rather than a known count | **While approved; pending:** `while (condition){}` uses truthiness. Do-while and a dedicated infinite-loop keyword remain missing |
| Break / continue | Exit a loop early or skip to the next iteration | **Approved; pending:** `break;`, `continue;`. Define nested-loop targets and cleanup |
| For-each and iterator protocol | Traverse elements without manually managing indices | **Syntax approved; pending:** `for (item, iterable){}` alongside the three-part for. Iterator protocol and scope remain open |
| Conditional expression and null fallback | Choose a value inline; provide a default for null | **Missing.** Existing `if` is statement-oriented; short-circuit rules need definition |
| Structural pattern matching | Match object shapes, bind fields, add guards, check all variants | **Missing extensions.** Basic value/range `match` is already specified, not forgotten |

Comprehensions (building a collection from a loop/filter), general spread/unpacking,
and concise indexing/slicing syntax are additional convenience candidates.
Method-based `.iget()` and `.slice()` are already specified, so these are not
new builtin container types. `else` exists in Rust but needs an explicit contract
entry. `null` is already specified and is reaffirmed; `inf` is newly approved,
with detailed semantics deferred. False, null, and zero are falsy; other values
are truthy unless additional falsy cases are defined. Empty values and other edge
cases need explicit decisions. Precedence and short circuiting remain open.

Function declarations now separate parameters with **commas**, with no final
semicolon (`func add(int a, int b){}`). Calls separate
arguments with **commas**, and both for forms use top-level **commas**. The
two loop forms can have distinct AST nodes; neither requires treating for as an
ordinary function or waiting for decorator overload resolution.

## 5. Functions, types, and families

### Function facilities

| Feature | Purpose | Marslang status |
| --- | --- | --- |
| Default and named arguments | Make optional configuration readable | **Missing.** Define evaluation timing and argument matching |
| Variadic user functions | Accept a variable number of arguments | **Missing general contract.** Variadic builtins do not establish user-function support |
| Anonymous functions / function values | Supply callbacks and return behavior from a function | **Missing complete contract.** Expression-bodied named functions are already specified |
| Return-type annotations and function types | Describe what functions return and which callbacks they accept | **Missing defined syntax.** Parameter and variable annotations already exist |
| Generators / yield | Produce items lazily while preserving function state | **Missing.** Depends on an iteration protocol and cleanup behavior |

### Type and object facilities

| Feature | Purpose | Marslang status |
| --- | --- | --- |
| Type aliases and user-defined generics | Name complex types; reuse typed containers/functions | **Missing.** Builtin `array[int]` does not imply general generic types |
| Enums / tagged variants | Represent token kinds and AST alternatives explicitly | **Missing.** Existing union parameters do not define tagged variants or exhaustive matching |
| Optional/result types and narrowing | Describe nullable values or success/failure and check them safely | **Missing dedicated model.** `null` and exceptions already exist in the design |
| Interfaces / traits and access control | State required methods; control public/private API | **Missing.** Families and inheritance alone do not supply these contracts |
| Static members, parent calls, properties, overrides | Share family-level state and define inheritance behavior precisely | **Missing or undecided beyond basic inheritance.** Define method lookup and initialization order first |

Decorators before `func`, including **`@Decorator.overload()`**, are approved and
pending. The confirmed package is **`std.Decorator`**, imported with
`takepkg std.Decorator;` and used as `Decorator`. Dispatch rules, decorator evaluation/order, and a
static-method decorator's name remain undecided. Python's `@staticmethod` was an
analogy, not an adopted name. Operator overloading, reflection, and general macros
remain candidates; function overloading does not imply operator overloading.
`hot` is not automatically a macro system.

## 6. Async/await and concurrency

**Explicitly deferred:** async/await and a language-level asynchronous execution
contract. A Python-inspired `std.asyncio` package is a tentative future direction,
not a finalized API. Emitting JavaScript does not automatically supply support.

| Part | What must be defined |
| --- | --- |
| Async function and await | How an operation can suspend and resume; where await is legal |
| Task/future value and scheduler | How work starts, progresses, and returns a result |
| Async I/O | Files, timers, network calls, and streams that can yield control |
| Errors, cancellation, and cleanup | How failures propagate; how tasks stop; how `then` runs on exit |
| Parallel workers and synchronization | A separate design for CPU parallelism, message passing, locks, or channels |

Async concurrency does not itself promise CPU parallelism. Python's
[coroutines and tasks documentation](https://docs.python.org/3/library/asyncio-task.html)
illustrates the distinction between coroutine functions, scheduled tasks, awaiting
results, and cancellation.

For Marslang, a possible later implementation is to lower async functions to
JavaScript promises and expose selected Node asynchronous services. That is a
backend proposal, not approved syntax. Settle ordinary errors, modules, and host
I/O first; a synchronous compiler can reach self-hosting without async/await.

## 7. Standard library, resources, and tooling

| Area | Gaps to address |
| --- | --- |
| Files and process services | `std.file` approved for file I/O, implementation deferred. Exact API, paths, directories, arguments, environment, and exit behavior remain to be defined |
| Common libraries | `std.network`, `std.regex`, `std.random`, `std.json` approved as planned packages; APIs pending. Networking subpackages such as `network.http` are tentative. Other string/date/math services remain candidates |
| Modules and packaging | Packages, relative paths, and `takepkg packageName = *;` approved. Relative spelling/base, exported-name selection, wildcard collisions, visibility, search paths, versions, and caching remain open |
| Resource management | Scoped file/socket cleanup and ownership rules; `then` covers cleanup blocks but not a complete resource protocol |
| Developer tools | Start with crude tools as needed; working syntax comes first. Formatter, linter, editor support, debugging, and distribution remain later work |

Native executables, bytecode, C interop, raw pointers, and explicit memory
allocation are separate platform choices. They are not required merely because
Marslang has C++-inspired syntax. The present backend deliberately requires Node.

## 8. Already requested, but incomplete in the reviewed Rust implementation

Do not redesign these as if the user forgot them. See the review and execution
tests for current evidence; the rows group related gaps rather than listing every bug.

| Existing requirement | Remaining work |
| --- | --- |
| Hot/cold/fixed and typed values | Actual hot substitution, immutable binding enforcement, scalar/union/collection type checks and accurate integer behavior |
| Arrays, sets, mutable pairs | Canonical `arr/set/pair` constructors, specified methods, restrictions on mutation, collection semantics |
| For/match/errors | For/also, match/range, run/handle/then, error raising and unpacking |
| Strings and reliable parsing | Strings containing comments, escapes/multiline syntax, source diagnostics, structured expressions and predictable scopes |
| Modules and tooling | Real `.mrs` module compilation/exports, verbose CLI and platform usability; replace source-replay REPL limitations deliberately |

Native bitwise operators (including shifts) and `:=` are excluded. An `ext`
extension-library namespace may ship alongside `std`; **`ext.bitopers`** with
functions such as `BIT_AND()` and `BIT_OR()` is a proposal. Its function signatures,
integer widths, signedness, and overflow rules are not defined. Arbitrary precision
integers and decimal arithmetic remain unspecified; `longint` is signed 64-bit.

The September 16 additions are design decisions awaiting implementation, not
newly passing compiler features. In particular, `fixed` copy propagation needs
explicit language/runtime tracking; JavaScript `const` alone cannot provide it.

## 9. How to turn an entry into a feature

1. Pick one concrete use case, such as looking up a compiler symbol by name.
2. Decide whether it needs syntax, a builtin type, a library, or only clearer rules.
3. Record syntax and semantics in the language contract, including errors and scope.
4. Add executable examples with expected results, then implement them.
5. Update this inventory's status when the contract or implementation changes.

Suggested first decision: define how a bare assignment behaves when the name
already exists locally or in an enclosing scope. That determines whether an
outer-scope keyword would help; `:=` remains excluded.
