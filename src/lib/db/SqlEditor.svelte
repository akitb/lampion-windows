<script lang="ts" module>
  import { acceptCompletion, autocompletion, completionStatus, selectedCompletionIndex, setSelectedCompletion, startCompletion, type CompletionContext } from "@codemirror/autocomplete";
  import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
  import { HighlightStyle, StreamLanguage, syntaxHighlighting, type StringStream } from "@codemirror/language";
  import { Compartment, EditorState, Prec, StateEffect, StateField } from "@codemirror/state";
  import { Decoration, EditorView, ViewPlugin, drawSelection, highlightActiveLineGutter, keymap, lineNumbers, type DecorationSet, type ViewUpdate } from "@codemirror/view";
  import { tags } from "@lezer/highlight";
  import { db } from "./dbstate.svelte";
  import { dbapi } from "./dbapi.svelte";
  import { query, runQuery } from "./model.svelte";
  import type { SyntaxWords } from "./types";

  // ---- Highlighting: the macOS editor's rules (SQLSyntax.swift) ----

  type Lex = { quote: string | null; comment: boolean };

  function sql(words: SyntaxWords) {
    const keywords = new Set(words.keywords);
    const functions = new Set(words.functions);
    const types = new Set(words.types);
    function token(stream: StringStream, state: Lex): string | null {
      if (state.comment) {
        if (stream.skipTo("*/")) {
          stream.pos += 2;
          state.comment = false;
        } else stream.skipToEnd();
        return "comment";
      }
      if (state.quote) {
        const q = state.quote;
        while (!stream.eol()) {
          const c = stream.next();
          if (c === "\\" && q !== "`") stream.next();
          else if (c === q) {
            if (stream.peek() === q) stream.next();
            else {
              state.quote = null;
              break;
            }
          }
        }
        return q === "`" ? "ident" : "string";
      }
      if (stream.eatSpace()) return null;
      if (stream.match(/^--(\s|$)/) || stream.peek() === "#") {
        stream.skipToEnd();
        return "comment";
      }
      if (stream.match("/*")) {
        state.comment = true;
        return token(stream, state);
      }
      const ch = stream.peek() ?? "";
      if (ch === "'" || ch === '"' || ch === "`") {
        stream.next();
        state.quote = ch;
        return token(stream, state);
      }
      const before = stream.string.charAt(stream.pos - 1);
      if (/\d/.test(ch) && !/[\p{L}\p{N}_$]/u.test(before)) {
        stream.match(/^[\p{L}\p{N}_$.]+/u);
        return "number";
      }
      const word = stream.match(/^[\p{L}\p{N}_$]+/u) as RegExpMatchArray | null;
      if (word) {
        const w = word[0].toUpperCase();
        if (functions.has(w) && stream.match(/^\s*\(/, false)) return "fn";
        if (types.has(w)) return "type";
        if (keywords.has(w)) return "keyword";
        return null;
      }
      stream.next();
      return null;
    }
    return StreamLanguage.define<Lex>({
      name: "sql",
      startState: () => ({ quote: null, comment: false }),
      token,
      tokenTable: { fn: tags.function(tags.variableName), type: tags.typeName, ident: tags.special(tags.string) },
    });
  }

  const colours = HighlightStyle.define([
    { tag: tags.keyword, color: "var(--db-purple)" },
    { tag: tags.function(tags.variableName), color: "var(--db-blue)" },
    { tag: tags.typeName, color: "var(--db-teal)" },
    { tag: tags.string, color: "var(--red)" },
    { tag: tags.special(tags.string), color: "var(--db-orange)" },
    { tag: tags.number, color: "var(--green)" },
    { tag: tags.comment, color: "var(--text-2)" },
  ]);

  let words: Promise<SyntaxWords> | null = null;
  const language = new Compartment();

  // ---- Marks: the last run's error (until the next edit) and unclosed quotes / brackets ----

  const mark = Decoration.mark({ class: "cm-sqlmark" });
  const setError = StateEffect.define<[number, number] | null>();
  const errorMark = StateField.define<DecorationSet>({
    create: () => Decoration.none,
    update(deco, tr) {
      for (const e of tr.effects)
        if (e.is(setError)) {
          const r = e.value;
          return r && r[1] > r[0] && r[1] <= tr.state.doc.length ? Decoration.set([mark.range(r[0], r[1])]) : Decoration.none;
        }
      return tr.docChanged ? Decoration.none : deco;
    },
    provide: (f) => EditorView.decorations.from(f),
  });

  const setLint = StateEffect.define<[number, number][]>();
  const lintMarks = StateField.define<DecorationSet>({
    create: () => Decoration.none,
    update(deco, tr) {
      deco = deco.map(tr.changes);
      for (const e of tr.effects)
        if (e.is(setLint)) {
          const len = tr.state.doc.length;
          deco = Decoration.set(e.value.filter(([a, b]) => b > a && b <= len).map(([a, b]) => mark.range(a, b)), true);
        }
      return deco;
    },
    provide: (f) => EditorView.decorations.from(f),
  });

  const lint = ViewPlugin.fromClass(
    class {
      timer = 0;
      constructor(view: EditorView) {
        this.check(view);
      }
      update(u: ViewUpdate) {
        if (u.docChanged) this.check(u.view);
      }
      check(view: EditorView) {
        clearTimeout(this.timer);
        this.timer = window.setTimeout(async () => {
          const doc = view.state.doc;
          const marks = await dbapi.syntaxMarks(doc.toString()).catch(() => []);
          if (view.state.doc === doc) view.dispatch({ effects: setLint.of(marks) });
        }, 250);
      }
      destroy() {
        clearTimeout(this.timer);
      }
    },
  );

  // ---- Completion (the lists come from the backend: keywords, tables, columns, aliases) ----

  const target = () => ({ database: query.database ?? db.database, table: db.table });

  async function suggest(ctx: CompletionContext) {
    const before = ctx.state.sliceDoc(Math.max(0, ctx.pos - 300), ctx.pos);
    const t = target();
    const r = await dbapi.complete(t.database, t.table, before, ctx.state.doc.toString()).catch(() => null);
    if (!r || ctx.aborted || !r.items.length) return null;
    // Only while a name is typed, after "name.", or on request (Tab).
    if (!ctx.explicit && r.prefixLen === 0 && !r.qualified) return null;
    return { from: ctx.pos - r.prefixLen, options: r.items.map((label) => ({ label })), filter: false };
  }

  /** Tab: accepts the suggestion; with the list closed, completes a single match or opens the list. */
  function tab(view: EditorView): boolean {
    if (completionStatus(view.state) === "active") {
      if (selectedCompletionIndex(view.state) === null) view.dispatch({ effects: setSelectedCompletion(0) });
      return acceptCompletion(view);
    }
    const sel = view.state.selection.main;
    if (!sel.empty) return false;
    const before = view.state.sliceDoc(Math.max(0, sel.head - 300), sel.head);
    if (!/[\p{L}\p{N}_$.`]$/u.test(before)) {
      view.dispatch(view.state.replaceSelection("\t"));
      return true;
    }
    const t = target();
    dbapi.complete(t.database, t.table, before, view.state.doc.toString()).then((r) => {
      if (view.state.selection.main.head !== sel.head || view.state.sliceDoc(Math.max(0, sel.head - 300), sel.head) !== before) return;
      if (r.items.length === 1) {
        const from = sel.head - r.prefixLen;
        view.dispatch({ changes: { from, to: sel.head, insert: r.items[0] }, selection: { anchor: from + r.items[0].length }, userEvent: "input.complete" });
      } else if (r.items.length > 1) startCompletion(view);
    }, () => {});
    return true;
  }

  const theme = EditorView.theme({
    "&": { height: "100%", fontSize: "13px", color: "var(--text)", backgroundColor: "var(--db-content)" },
    "&.cm-focused": { outline: "none" },
    ".cm-scroller": { fontFamily: "var(--mono)", lineHeight: "1.25", overflow: "auto" },
    ".cm-content": { padding: "10px 0", caretColor: "var(--accent)" },
    ".cm-line": { padding: "0 6px 0 11px" },
    ".cm-cursor, .cm-dropCursor": { borderLeftColor: "var(--accent)", borderLeftWidth: "1.5px" },
    "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection": { backgroundColor: "color-mix(in srgb, var(--accent) 35%, transparent)" },
    ".cm-gutters": { backgroundColor: "var(--db-content)", border: "none", color: "var(--text-3)" },
    ".cm-lineNumbers .cm-gutterElement": {
      minWidth: "34px",
      padding: "0 8px 0 0",
      fontFamily: "var(--font)",
      fontSize: "10.5px",
      lineHeight: "16.25px",
      fontVariantNumeric: "tabular-nums",
    },
    ".cm-activeLineGutter": { backgroundColor: "transparent", color: "var(--text-2)" },
    ".cm-sqlmark": {
      backgroundColor: "color-mix(in srgb, var(--red) 25%, transparent)",
      textDecoration: "underline dotted var(--red)",
      textUnderlineOffset: "3px",
    },
    ".cm-tooltip": { border: "1px solid var(--border-strong)", borderRadius: "8px", backgroundColor: "var(--db-menu)", boxShadow: "0 8px 24px rgba(0,0,0,0.35)", overflow: "hidden" },
    ".cm-tooltip.cm-tooltip-autocomplete > ul": { fontFamily: "var(--mono)", fontSize: "12px", maxHeight: "168px", minWidth: "180px", maxWidth: "460px", padding: "4px 0" },
    ".cm-tooltip.cm-tooltip-autocomplete > ul > li": { padding: "0 10px", lineHeight: "20px", height: "20px", color: "var(--text)" },
    ".cm-tooltip.cm-tooltip-autocomplete > ul > li[aria-selected]": { backgroundColor: "var(--accent)", color: "var(--accent-text)" },
  });

  function extensions() {
    return [
      lineNumbers(),
      highlightActiveLineGutter(),
      drawSelection(),
      history(),
      language.of([]),
      syntaxHighlighting(colours),
      errorMark,
      lintMarks,
      lint,
      autocompletion({ override: [suggest], activateOnTyping: true, selectOnOpen: false, icons: false, closeOnBlur: true }),
      Prec.highest(
        keymap.of([
          { key: "Tab", run: tab },
          { key: "Mod-r", run: () => (runQuery(), true), preventDefault: true },
          { key: "Mod-Enter", run: () => (runQuery(), true) },
        ]),
      ),
      keymap.of([...historyKeymap, ...defaultKeymap]),
      EditorView.updateListener.of((u) => {
        if (u.docChanged) query.text = u.state.doc.toString();
      }),
      EditorView.contentAttributes.of({ "aria-label": "SQL editor", spellcheck: "false" }),
      theme,
    ];
  }
</script>

<script lang="ts">
  // The Query tab's editor: line numbers, highlighting, suggestions while typing (Tab accepts), and
  // the last error marked until the next edit. Its state survives switching tabs.
  import { onMount } from "svelte";

  let host: HTMLDivElement | undefined = $state();
  let view: EditorView | null = null;
  let seenVersion = 0;

  onMount(() => {
    if (!host) return;
    seenVersion = query.version;
    const v = new EditorView({ parent: host, state: query.editor ?? EditorState.create({ doc: query.text, extensions: extensions() }) });
    view = v;
    words ??= dbapi.syntax();
    words.then((w) => { if (view === v) v.dispatch({ effects: language.reconfigure(sql(w)) }); }, () => {});
    return () => {
      query.editor = v.state;
      v.destroy();
      view = null;
    };
  });

  // Text set from outside (History, Clear).
  $effect(() => {
    const text = query.text;
    if (view && view.state.doc.toString() !== text) view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text } });
  });

  // A finished run marks its error (or clears the old mark).
  $effect(() => {
    const v = query.version;
    const range = query.errorRange;
    if (!view || v === seenVersion) return;
    seenVersion = v;
    view.dispatch({ effects: setError.of(range) });
  });
</script>

<div class="editor" bind:this={host}></div>

<style>
  .editor { height: 100%; overflow: hidden; }
  .editor :global(.cm-editor) { height: 100%; }
</style>
