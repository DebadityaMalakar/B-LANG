# B Language Support

Syntax highlighting and a lightweight language server for the B language MVP.

## Features
- Syntax highlighting for `.b` files.
- Diagnostics for unterminated comments/strings and mismatched delimiters.
- Hover help for B keywords, builtins, operators, and numeric literals.
- Document symbols for top-level functions and globals.

## Build

```bash
npm install
npm run compile
```

Launch the extension in VS Code with the "Run Extension" debug target.
