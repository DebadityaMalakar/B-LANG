import {
  createConnection,
  ProposedFeatures,
  TextDocuments,
  Diagnostic,
  DiagnosticSeverity,
  Hover,
  SymbolKind,
  TextDocumentSyncKind,
  DocumentSymbol
} from "vscode-languageserver/node";
import { TextDocument } from "vscode-languageserver-textdocument";

const connection = createConnection(ProposedFeatures.all);
const documents: TextDocuments<TextDocument> = new TextDocuments(TextDocument);

const keywords = new Map<string, string>([
  ["auto", "Declare local (auto) variables."],
  ["extrn", "Declare external/global variables."],
  ["if", "Conditional statement."],
  ["else", "Alternate branch for if."],
  ["while", "Loop while condition is non-zero."],
  ["switch", "Multi-branch selection."],
  ["case", "Case label inside switch."],
  ["default", "Default label inside switch."],
  ["break", "Exit loop or switch."],
  ["return", "Return from function."],
  ["goto", "Jump to label."],
]);

const builtinFunctions = new Map<string, { signature: string; description: string }>([
  ["getchar", { signature: "getchar()", description: "Read one character from input and return its code." }],
  ["putchar", { signature: "putchar(c)", description: "Write one character; returns the value written." }],
  ["putnumbs", { signature: "putnumbs(n)", description: "Print a decimal integer." }],
  ["printf", { signature: "printf(fmt, ...)", description: "Formatted output (supports %d, %o, %c, %s)." }],
  ["getstr", { signature: "getstr(s)", description: "Read a line into vector s and append \"*e\" terminator." }],
  ["putstr", { signature: "putstr(s)", description: "Write characters from vector s until terminator." }],
  ["openr", { signature: "openr(u, s)", description: "Open input unit u from filename s (MVP may be stubbed)." }],
  ["openw", { signature: "openw(u, s)", description: "Open output unit u to filename s (MVP may be stubbed)." }],
  ["flush", { signature: "flush()", description: "Flush output buffers (MVP may be stubbed)." }],
  ["reread", { signature: "reread()", description: "Reset input stream (MVP may be stubbed)." }],
  ["system", { signature: "system(s)", description: "Execute a system command string (MVP may be stubbed)." }],
  ["char", { signature: "char(s, n)", description: "Return the n-th character of vector s." }],
  ["lchar", { signature: "lchar(s, n, c)", description: "Set the n-th character of vector s and return c." }],
  ["concat", { signature: "concat(a, b1, ..., b10)", description: "Concatenate strings into vector a." }],
  ["getarg", { signature: "getarg(n)", description: "Return the n-th command line argument." }],
  ["getvec", { signature: "getvec(n)", description: "Allocate a vector of size n and return its address." }],
  ["rlsvec", { signature: "rlsvec(v)", description: "Release a vector previously allocated with getvec." }],
  ["exit", { signature: "exit()", description: "Terminate execution." }],
  ["nargs", { signature: "nargs()", description: "Return number of arguments to current function." }]
]);

const operatorDocs = new Map<string, string>([
  ["+", "Addition."],
  ["-", "Subtraction or unary negation."],
  ["*", "Multiplication or indirection (dereference)."],
  ["/", "Division."],
  ["%", "Remainder."],
  ["<<", "Left shift."],
  [">>", "Right shift."],
  ["<", "Less-than comparison."],
  ["<=", "Less-than or equal comparison."],
  [">", "Greater-than comparison."],
  [">=", "Greater-than or equal comparison."],
  ["==", "Equality comparison."],
  ["!=", "Inequality comparison."],
  ["&", "Bitwise AND or address-of."],
  ["|", "Bitwise OR."],
  ["^", "Bitwise XOR."],
  ["~", "Bitwise NOT."],
  ["!", "Logical NOT."],
  ["&&", "Logical AND (short-circuit)."],
  ["||", "Logical OR (short-circuit)."],
  ["=", "Assignment."],
  ["+=", "Add and assign."],
  ["-=", "Subtract and assign."],
  ["*=", "Multiply and assign."],
  ["/=", "Divide and assign."],
  ["%=", "Remainder and assign."],
  ["<<=", "Left shift and assign."],
  [">>=", "Right shift and assign."],
  ["&=", "Bitwise AND and assign."],
  ["|=", "Bitwise OR and assign."],
  ["^=", "Bitwise XOR and assign."],
  ["++", "Increment."],
  ["--", "Decrement."],
  ["?", "Conditional operator (ternary)."],
  [":", "Conditional operator separator."]
]);

connection.onInitialize(() => {
  return {
    capabilities: {
      textDocumentSync: TextDocumentSyncKind.Incremental,
      hoverProvider: true,
      documentSymbolProvider: true
    }
  };
});

documents.onDidOpen((event) => {
  validateTextDocument(event.document);
});

documents.onDidChangeContent((change) => {
  validateTextDocument(change.document);
});

connection.onHover((params): Hover | null => {
  const doc = documents.get(params.textDocument.uri);
  if (!doc) {
    return null;
  }
  const text = doc.getText();
  const { tokens } = lex(text);
  const token = getTokenAtPosition(tokens, params.position.line, params.position.character);
  if (!token) {
    return null;
  }

  if (token.type === "ident") {
    const keywordInfo = keywords.get(token.value);
    if (keywordInfo) {
      return buildHover(`**${token.value}**`, [`Keyword - ${keywordInfo}`]);
    }

    const builtinInfo = builtinFunctions.get(token.value);
    if (builtinInfo) {
      return buildHover(`**${token.value}**`, ["Builtin function.", `\`${builtinInfo.signature}\``, builtinInfo.description]);
    }

    const topLevel = collectTopLevelKinds(tokens).get(token.value);
    if (topLevel) {
      return buildHover(`**${token.value}**`, [formatSymbolKind(topLevel)]);
    }

    return null;
  }

  if (token.type === "number") {
    const info = describeNumberLiteral(token.value);
    return info ? buildHover(`\`${token.value}\``, info) : null;
  }

  if (token.type === "symbol") {
    const opInfo = operatorDocs.get(token.value);
    if (opInfo) {
      return buildHover(`\`${token.value}\``, [opInfo]);
    }
  }

  return null;
});

connection.onDocumentSymbol((params): DocumentSymbol[] => {
  const doc = documents.get(params.textDocument.uri);
  if (!doc) {
    return [];
  }
  return collectSymbols(doc.getText());
});

async function validateTextDocument(doc: TextDocument): Promise<void> {
  const text = doc.getText();
  const result = lex(text);
  const diagnostics: Diagnostic[] = [];

  for (const issue of result.issues) {
    diagnostics.push({
      severity: issue.severity,
      range: {
        start: { line: issue.line, character: issue.character },
        end: { line: issue.line, character: issue.character + 1 }
      },
      message: issue.message,
      source: "b-lang"
    });
  }

  connection.sendDiagnostics({ uri: doc.uri, diagnostics });
}

documents.listen(connection);
connection.listen();

type TokenType = "ident" | "number" | "symbol";

interface Token {
  type: TokenType;
  value: string;
  line: number;
  character: number;
}

interface Issue {
  message: string;
  line: number;
  character: number;
  severity: DiagnosticSeverity;
}

function lex(text: string): { tokens: Token[]; issues: Issue[] } {
  const tokens: Token[] = [];
  const issues: Issue[] = [];
  let line = 0;
  let character = 0;
  let index = 0;
  const stack: { ch: string; line: number; character: number }[] = [];

  const pushIssue = (message: string, line: number, character: number, severity: DiagnosticSeverity) => {
    issues.push({ message, line, character, severity });
  };

  const advance = (count = 1) => {
    for (let i = 0; i < count; i++) {
      const ch = text[index];
      index += 1;
      if (ch === "\n") {
        line += 1;
        character = 0;
      } else {
        character += 1;
      }
    }
  };

  while (index < text.length) {
    const ch = text[index];

    if (ch === " " || ch === "\t" || ch === "\r" || ch === "\n") {
      advance();
      continue;
    }

    if (ch === "/" && text[index + 1] === "*") {
      const start = { line, character };
      advance(2);
      let closed = false;
      while (index < text.length) {
        if (text[index] === "*" && text[index + 1] === "/") {
          advance(2);
          closed = true;
          break;
        }
        advance();
      }
      if (!closed) {
        pushIssue("Unterminated comment", start.line, start.character, DiagnosticSeverity.Error);
      }
      continue;
    }

    if (ch === "\"") {
      const start = { line, character };
      advance();
      let closed = false;
      while (index < text.length) {
        const current = text[index];
        if (current === "\"") {
          advance();
          closed = true;
          break;
        }
        if (current === "\\") {
          advance();
          if (index < text.length && text[index] === "*") {
            advance(2);
          } else if (index < text.length) {
            advance();
          }
          continue;
        }
        advance();
      }
      if (!closed) {
        pushIssue("Unterminated string literal", start.line, start.character, DiagnosticSeverity.Error);
      }
      continue;
    }

    if (ch === "'") {
      const start = { line, character };
      advance();
      if (index >= text.length) {
        pushIssue("Unterminated character literal", start.line, start.character, DiagnosticSeverity.Error);
        break;
      }
      if (text[index] === "\\") {
        advance();
        if (index < text.length && text[index] === "*") {
          advance(2);
        } else if (index < text.length) {
          advance();
        }
      } else {
        advance();
      }
      if (text[index] !== "'") {
        pushIssue("Unterminated character literal", start.line, start.character, DiagnosticSeverity.Error);
        continue;
      }
      advance();
      continue;
    }

    if (isIdentStart(ch)) {
      const start = { line, character };
      let value = "";
      while (index < text.length && isIdentContinue(text[index])) {
        value += text[index];
        advance();
      }
      tokens.push({ type: "ident", value, line: start.line, character: start.character });
      continue;
    }

    if (isDigit(ch)) {
      const start = { line, character };
      let value = "";
      while (index < text.length && isDigit(text[index])) {
        value += text[index];
        advance();
      }
      tokens.push({ type: "number", value, line: start.line, character: start.character });
      continue;
    }

    const symbol = readSymbol(text, index);
    if (symbol) {
      tokens.push({ type: "symbol", value: symbol, line, character });
      if (symbol === "{" || symbol === "(" || symbol === "[") {
        stack.push({ ch: symbol, line, character });
      } else if (symbol === "}" || symbol === ")" || symbol === "]") {
        const expected = matching(symbol);
        const top = stack.pop();
        if (!top || top.ch !== expected) {
          pushIssue("Mismatched delimiter", line, character, DiagnosticSeverity.Error);
        }
      }
      advance(symbol.length);
      continue;
    }

    if (ch.charCodeAt(0) < 32 || ch.charCodeAt(0) > 126) {
      pushIssue("Invalid character", line, character, DiagnosticSeverity.Warning);
      advance();
      continue;
    }

    pushIssue(`Unexpected character '${ch}'`, line, character, DiagnosticSeverity.Error);
    advance();
  }

  while (stack.length > 0) {
    const unclosed = stack.pop();
    if (unclosed) {
      pushIssue("Unclosed delimiter", unclosed.line, unclosed.character, DiagnosticSeverity.Error);
    }
  }

  return { tokens, issues };
}

function readSymbol(text: string, index: number): string | null {
  const two = text.slice(index, index + 2);
  const three = text.slice(index, index + 3);
  const symbols = [
    "<<=",
    ">>=",
    "++",
    "--",
    "<<",
    ">>",
    "<=",
    ">=",
    "==",
    "!=",
    "+=",
    "-=",
    "*=",
    "/=",
    "%=",
    "&=",
    "|=",
    "^=",
    "&&",
    "||"
  ];

  if (symbols.includes(three)) {
    return three;
  }
  if (symbols.includes(two)) {
    return two;
  }

  const single = text[index];
  if ("+-*/%<>=!&|^~?:,;(){}[]".includes(single)) {
    return single;
  }
  return null;
}

function matching(ch: string): string {
  switch (ch) {
    case ")":
      return "(";
    case "]":
      return "[";
    case "}":
      return "{";
    default:
      return "";
  }
}

function isIdentStart(ch: string): boolean {
  return /[A-Za-z_]/.test(ch);
}

function isIdentContinue(ch: string): boolean {
  return /[A-Za-z0-9_]/.test(ch);
}

function isDigit(ch: string): boolean {
  return /[0-9]/.test(ch);
}

function buildHover(title: string, lines: string[]): Hover {
  return {
    contents: {
      kind: "markdown",
      value: [title, "", ...lines].join("\n")
    }
  };
}

function getTokenAtPosition(tokens: Token[], line: number, character: number): Token | null {
  for (const token of tokens) {
    if (token.line !== line) {
      continue;
    }
    const end = token.character + token.value.length;
    if (character >= token.character && character < end) {
      return token;
    }
  }
  return null;
}

function describeNumberLiteral(value: string): string[] | null {
  if (!/^\d+$/.test(value)) {
    return null;
  }
  if (value.length > 1 && value.startsWith("0")) {
    if (/^0[0-7]+$/.test(value)) {
      const decValue = Number.parseInt(value, 8);
      return ["Octal literal.", `Value: ${decValue} (decimal).`];
    }
    return ["Octal literal (invalid digits).", "B treats leading zero as octal; digits must be 0-7."];
  }

  const decValue = Number.parseInt(value, 10);
  return ["Decimal literal.", `Value: ${decValue}.`, `Octal: ${decValue.toString(8)}.`];
}

function collectSymbols(text: string): DocumentSymbol[] {
  const { tokens } = lex(text);
  return collectSymbolsFromTokens(tokens);
}

function collectSymbolsFromTokens(tokens: Token[]): DocumentSymbol[] {
  const symbols: DocumentSymbol[] = [];
  let braceDepth = 0;

  for (let i = 0; i < tokens.length; i += 1) {
    const token = tokens[i];
    if (token.type === "symbol") {
      if (token.value === "{") {
        braceDepth += 1;
      } else if (token.value === "}") {
        braceDepth = Math.max(0, braceDepth - 1);
      }
      continue;
    }

    if (braceDepth !== 0 || token.type !== "ident") {
      continue;
    }

    const next = tokens[i + 1];
    if (!next) {
      continue;
    }

    if (next.type === "symbol" && next.value === "(") {
      symbols.push(toSymbol(token, SymbolKind.Function));
      continue;
    }

    if (next.type === "symbol" && (next.value === ";" || next.value === "[")) {
      symbols.push(toSymbol(token, SymbolKind.Variable));
    }
  }

  return symbols;
}

function collectTopLevelKinds(tokens: Token[]): Map<string, SymbolKind> {
  const kinds = new Map<string, SymbolKind>();
  let braceDepth = 0;

  for (let i = 0; i < tokens.length; i += 1) {
    const token = tokens[i];
    if (token.type === "symbol") {
      if (token.value === "{") {
        braceDepth += 1;
      } else if (token.value === "}") {
        braceDepth = Math.max(0, braceDepth - 1);
      }
      continue;
    }

    if (braceDepth !== 0 || token.type !== "ident") {
      continue;
    }

    const next = tokens[i + 1];
    if (!next || next.type !== "symbol") {
      continue;
    }

    if (next.value === "(") {
      kinds.set(token.value, SymbolKind.Function);
    } else if (next.value === ";" || next.value === "[") {
      kinds.set(token.value, SymbolKind.Variable);
    }
  }

  return kinds;
}

function formatSymbolKind(kind: SymbolKind): string {
  switch (kind) {
    case SymbolKind.Function:
      return "Function declaration.";
    case SymbolKind.Variable:
      return "Global variable declaration.";
    default:
      return "Symbol declaration.";
  }
}

function toSymbol(token: Token, kind: SymbolKind): DocumentSymbol {
  const start = { line: token.line, character: token.character };
  const end = { line: token.line, character: token.character + token.value.length };
  return {
    name: token.value,
    kind,
    range: { start, end },
    selectionRange: { start, end }
  };
}
