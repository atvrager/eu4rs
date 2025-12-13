# eu4txt (Library)

`eu4txt` is a specialized parser library designed to read the custom script format used by EU4 game files.

## The Format

The format is a key-value structure similar to JSON or Lua tables but with lax syntax rules.

```text
keys = {
    key = value
    list = { 1 2 3 }
    nested = {
        subkey = "string value"
    }
}
```

## Parsing Strategy

We use a **handwritten recursive descent parser** to tokenize and parse this text into Rust data structures.

-   **Lenient Parsing**: The parser is designed to handle common syntax quirks found in modded files.
-   **Deserialization**: utilizing `serde` to deserialize the parsed AST directly into strongly-typed Rust structs defined in `eu4data`.

## Usage

```rust
use eu4txt::de::from_str;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct MyData {
    key: String,
    value: i32,
}

let text = r#"
    key = "foo"
    value = 10
"#;

let data: MyData = from_str(text).unwrap();
```
