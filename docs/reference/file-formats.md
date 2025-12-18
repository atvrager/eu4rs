# EU4 File Format Documentation

This document describes the structure, syntax, and quirks of the text-based file formats used by *Europa Universalis IV* (EU4), which `eu4rs` assumes when parsing.

## 1. Encoding

*   **Standard**: `Windows-1252` (CP1252).
*   **Behavior**: Most files use single-byte characters. `eu4txt` automatically handles decoding this to UTF-8.
*   **Note**: Some modern files or user-mods might use UTF-8 with BOM, but the vanilla game predominantly uses Windows-1252.

## 2. Basic Syntax

The format is a custom key-value serialization format, loosely similar to JSON but with significant differences.

### Assignments
Data is structured as assignments: `key = value`.

```txt
max_provinces = 3000
owner = SWE
```

### Scopes (Objects)
Objects are delimited by braces `{ ... }` and contain a list of assignments or values.

```txt
country = {
    tag = SWE
    color = { 0 0 255 }
}
```

### Lists
Lists are implicitly defined by multiple values appearing without keys, usually separated by whitespace.

```txt
# A list of numbers (RGB color)
color = { 0 0 255 }

# A list of strings
historical_ideas = {
    defensive_ideas
    economic_ideas
}
```

### Comments
Comments start with `#` and continue to the end of the line.

```txt
max_provinces = 3000 # This is a comment
```

## 3. Data Types

*   **Identifier**: Unquoted strings (e.g., `SWE`, `max_provinces`). Can contain alphanumeric characters and underscores.
*   **String**: Double-quoted text (e.g., `"Sweden"`). Used for localization keys or names with spaces.
*   **Integer**: Whole numbers (e.g., `1444`).
*   **Float**: Decimal numbers (e.g., `12.50`, `0.5`).
*   **Date**: Often formatted as `YYYY.MM.DD` (e.g., `1444.11.11`). These are typically parsed as string identifiers by the tokenizer and must be handled by the game logic.
*   **Boolean**: `yes` and `no` are treated as identifiers but often equate to boolean true/false in logic.

## 4. Parser Quirks & Edge Cases

The EU4 format is not strictly standardized, leading to several "quirks" that `eu4txt` must handle.

### Quoted Keys in Assignments
While most keys are identifiers, some files (especially `common/countries/*.txt`) use quoted strings as keys in assignments.

**Example**:
```txt
"Angatupyry #0" = {
    name = "Angatupyry"
    dynasty = "Guaranii"
}
```

The parser supports both `Identifier = Value` and `"String" = Value`.

### Mixed Lists/Objects
A scope can contain *both* assignments and bare values, effectively acting as both a map and a list.

**Example**:
```txt
history = {
    owner = SWE    # Key-Value
    1444.11.11     # Bare value (Date)
    add_core = FIN # Key-Value
}
```

### Duplicate Keys
Keys are **not unique**. A scope can contain multiple assignments with the same key.

**Example**:
```txt
# Valid EU4 syntax
add_core = SWE
add_core = FIN
```
Game logic typically treats this as a list of values for that key, or last-write-wins depending on the context.

### "Empty" Assignments
Files may occasionally contain valid grammar that maps to "empty" or "null" logic, such as empty braces `{}`.

### Unconsumed Tokens
The parser must be robust against files ending with trailing whitespace or comments. `eu4txt` enforces that all tokens are consumed to ensure valid parsing, returning an error if significant tokens remain.
