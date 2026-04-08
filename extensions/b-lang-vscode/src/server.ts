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
  const word = getWordAtPosition(doc, params.position.line, params.position.character);
  if (!word) {
    return null;
  }
  const info = keywords.get(word);
  if (!info) {
    return null;
  }
  return {
    contents: {
      kind: "markdown",
      value: `**${word}**\n\n${info}`
    }
  };
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

function getWordAtPosition(doc: TextDocument, line: number, character: number): string | null {
  const text = doc.getText();
  const offset = doc.offsetAt({ line, character });
  const before = text.slice(0, offset);
  const after = text.slice(offset);
  const beforeMatch = before.match(/[A-Za-z_][A-Za-z0-9_]*$/);
  const afterMatch = after.match(/^[A-Za-z0-9_]*/);
  if (!beforeMatch) {
    return null;
  }
  return beforeMatch[0] + (afterMatch ? afterMatch[0] : "");
}

function collectSymbols(text: string): DocumentSymbol[] {
  const { tokens } = lex(text);
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
