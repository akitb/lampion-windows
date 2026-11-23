// Colouring for the read-only CREATE statement, with the macOS editor's rules: `name` identifiers,
// 'text' strings, numbers, comments, and the editor's keywords, functions (before "(") and types.
// The word lists come once from part A's `db_syntax`; without them only the punctuation-based kinds colour.
import { dbapi } from "../dbapi.svelte";

export type TokenKind = "keyword" | "function" | "type" | "string" | "number" | "comment" | "identifier";
export interface Segment {
  text: string;
  kind: TokenKind | null;
}

export interface Words {
  keywords: Set<string>;
  functions: Set<string>;
  types: Set<string>;
}

let words: Promise<Words> | null = null;

/** The highlighting words, loaded once (an empty set when the command is not there). */
export function syntaxWords(): Promise<Words> {
  words ??= dbapi
    .syntax()
    .then((w) => ({ keywords: new Set(w.keywords), functions: new Set(w.functions), types: new Set(w.types) }))
    .catch(() => ({ keywords: new Set<string>(), functions: new Set<string>(), types: new Set<string>() }));
  return words;
}

const isIdent = (c: string | undefined) => !!c && (/[\p{L}\p{N}]/u.test(c) || c === "_" || c === "$");
const isDigit = (c: string | undefined) => !!c && c >= "0" && c <= "9";

/** The statement as coloured and plain segments, in order (joined they give `sql` back). */
export function highlight(sql: string, w: Words): Segment[] {
  const out: Segment[] = [];
  const n = sql.length;
  let plain = 0;
  const add = (start: number, end: number, kind: TokenKind) => {
    if (start > plain) out.push({ text: sql.slice(plain, start), kind: null });
    out.push({ text: sql.slice(start, end), kind });
    plain = end;
  };
  const lineEnd = (from: number) => {
    const j = sql.indexOf("\n", from);
    return j < 0 ? n : j;
  };
  let i = 0;
  while (i < n) {
    const c = sql[i];
    if ((c === "-" && sql[i + 1] === "-" && (i + 2 >= n || " \t\n".includes(sql[i + 2]))) || c === "#") {
      const j = lineEnd(i);
      add(i, j, "comment");
      i = j;
    } else if (c === "/" && sql[i + 1] === "*") {
      const close = sql.indexOf("*/", i + 2);
      const j = close < 0 ? n : close + 2;
      add(i, j, "comment");
      i = j;
    } else if (c === "'" || c === '"' || c === "`") {
      let j = i + 1;
      let closed = false;
      while (j < n) {
        if (sql[j] === "\\" && c !== "`") {
          j += 2;
          continue;
        }
        if (sql[j] === c) {
          if (sql[j + 1] === c) {
            j += 2;
            continue;
          }
          closed = true;
          j++;
          break;
        }
        j++;
      }
      // An unclosed quote leaves the rest of its line plain, like the editor's error mark.
      if (closed) add(i, Math.min(j, n), c === "`" ? "identifier" : "string");
      i = closed ? Math.min(j, n) : Math.max(lineEnd(i), i + 1);
    } else if (isDigit(c) && !isIdent(sql[i - 1])) {
      let j = i;
      while (j < n && (isIdent(sql[j]) || sql[j] === ".")) j++;
      add(i, j, "number");
      i = j;
    } else if (isIdent(c)) {
      let j = i;
      while (j < n && isIdent(sql[j])) j++;
      const word = sql.slice(i, j).toUpperCase();
      let k = j;
      while (sql[k] === " " || sql[k] === "\t") k++;
      if (w.functions.has(word) && sql[k] === "(") add(i, j, "function");
      else if (w.types.has(word)) add(i, j, "type");
      else if (w.keywords.has(word)) add(i, j, "keyword");
      i = j;
    } else {
      i++;
    }
  }
  if (plain < n) out.push({ text: sql.slice(plain), kind: null });
  return out;
}
