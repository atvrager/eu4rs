---
description: Improve parsing coverage for a data category to 100%
---

# Improve Data Coverage

Execute the coverage improvement workflow defined in `.agent/workflows/improve_coverage.md`.

## Instructions

Find a low-coverage data type and bring it to 100% parsed coverage:

1. **Read workflow**: Follow all steps in `.agent/workflows/improve_coverage.md`
2. **Identify candidate**: Run `cargo xtask coverage` to find low-coverage categories
3. **Inspect missing fields**: Check `docs/supported_fields.md`
4. **Implement fields**: Generate types, expose API, register for coverage
5. **Verify improvements**: Re-run coverage update
6. **CI check**: Run `cargo xtask ci`
7. **Commit**: Document coverage improvement

This is a structured approach to systematically improve data parsing completeness.