# eu4tokens

Extract binary token mappings from EU4 game files.

## Status: Partial Implementation

This tool extracts **string identifiers** from the EU4 game binary but **does not yet recover the correct token IDs**. The extracted tokens file won't work with eu4save/jomini for parsing ironman saves.

### What Works
- Finds 2800+ field name strings in the ELF binary
- Outputs in pdx-tools compatible format
- Good for understanding what fields exist

### What Doesn't Work
- Token IDs are assigned sequentially (0x0000, 0x0001, ...)
- Actual token IDs in saves are different (e.g., 0x2f3f, 0x284d)
- Binary saves parsed with this token file will fail

## Usage

```bash
# Extract tokens (strings only, IDs are heuristic)
cargo run -p eu4tokens -- derive /path/to/eu4

# Use existing tokens file
cargo run -p eu4tokens -- use /path/to/tokens.txt
```

## Token File Format

```
0x0000 field_name_one
0x0001 field_name_two
0x0002 another_field
```

## Alternatives for Working Tokens

1. **Rakaly CLI**: Can melt saves even with unknown tokens
   ```bash
   cargo install rakaly
   rakaly melt --unknown-key stringify save.eu4 > melted.txt
   ```

2. **PDX-Unlimiter**: Can extract tokens from game executable

3. **Community**: Some token files exist in modding communities

## Technical Background

EU4's binary save format uses 16-bit token IDs to represent field names:
- `0x2f3f` = equals operator, followed by value
- The mapping from ID → string is embedded in the game binary

The challenge is that the mapping is likely stored as:
- A hash table (token ID computed from string hash)
- A sorted array (binary search at runtime)
- Inline in generated code

Our heuristic approach finds the strings but can't recover the ID assignment.

## Platform Support

- **Linux/ELF**: Implemented (finds strings)
- **Windows/PE**: Graceful error message

## Future Work

- Runtime hooking to intercept token table
- Pattern matching for hash table structures
- Collaboration with existing reverse engineering efforts
