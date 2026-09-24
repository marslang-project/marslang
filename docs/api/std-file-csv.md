# std.file.csv

CSV tables, as [RFC 4180](https://www.rfc-editor.org/rfc/rfc4180) defines them
and as spreadsheets save them: fields separated by commas, quoted when they hold
a comma, a quote, or a line break, with a quote inside written twice.

```mars
takepkg std.file.csv;

func m{
    people = csv.parse_records("name,age\nAda,36\nGrace,45\n");
    for (person, people){ out(person.get("name"), "is", person.get("age")); }

    out(csv.format(arr(arr("item", "note"), arr("tea", "hot, strong"))));
}
```

```text
Ada is 36
Grace is 45
item,note
tea,"hot, strong"
```

| Function | Reads or writes |
| --- | --- |
| `read(path)`, `parse(text)` | Rows: an array of arrays of strings |
| `write(path, rows)`, `format(rows)` | Rows: an array of arrays |
| `read_records(path)`, `parse_records(text)` | Records: an array of maps keyed by the first row |
| `write_records(path, records)`, `format_records(records)` | Records: an array of maps |

`read` and `write` use [std.file](std-file.md), so a missing file raises
`file.NotFoundError` and writing is all or nothing.

## Reading

Every field reads as a string, because CSV does not say which fields are
numbers; convert with `int()` or `float()`. Both `\n` and `\r\n` end a row,
blank lines are skipped, and a byte-order mark at the start, which Excel writes,
is ignored. Rows may have different lengths.

Records need every row to match the header, and the header to name each column
once. Mistakes raise `SyntaxError` with the line, and the file when there is
one:

```text
SyntaxError: people.csv: CSV line 4: 3 fields, but the header has 2
SyntaxError: CSV line 1: this quoted field is never closed
SyntaxError: CSV line 2: a quote inside an unquoted field; quote the whole field and write the quote twice
```

## Writing

Strings are written as they are, and quoted only when they must be. Numbers,
booleans, and dates are written as `out` prints them, and `null` as an empty
field. Arrays,
maps, and other containers raise `TypeError`, since a field holds one value.
Each row ends in `\n`, which every spreadsheet and CSV reader accepts.

`format_records` takes its header from the first record's keys, in order. A
later record without one of those keys gets an empty field; a record with a key
the first does not have raises `RangeError`, since its value would be lost.

## Other separators

Many European spreadsheets separate fields with `;`, and TSV files use a tab.
`csv.with_separator` gives the same eight functions for any one character:

```mars
semi = csv.with_separator(";");
rows = semi.read("export.csv");

tsv = csv.with_separator("\t");
tsv.write("table.tsv", rows);
```

The separator cannot be a quote or a line break. Quoting works the same way,
so a field holding the separator is quoted.

The wrapper is written in Marslang ([std/file/csv.mars](../../std/file/csv.mars));
reading and writing CSV text is native, in [std/rs/file.rs](../../std/rs/file.rs).
