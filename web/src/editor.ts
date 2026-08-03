import { EditorView } from "codemirror";
import { markdown } from "@codemirror/lang-markdown";
import {
  defaultKeymap,
  history,
  historyKeymap,
  indentLess,
  indentMore,
} from "@codemirror/commands";
import { defaultHighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { Prec } from "@codemirror/state";
import { drawSelection, highlightSpecialChars, keymap } from "@codemirror/view";
import { vim, Vim } from "@replit/codemirror-vim";

// `minimalSetup` inlined, minus the mac-only emacs-style `Ctrl-<letter>`
// bindings baked into `defaultKeymap` (Ctrl-d deleteCharForward, Ctrl-k
// deleteToLineEnd, Ctrl-o splitLine, …). Vim owns those chords, and whenever
// its own handler declines one (e.g. `<C-d>` scroll in a doc too short to
// scroll) the emacs binding used to fire and eat text instead.
const setup = [
  highlightSpecialChars(),
  history(),
  drawSelection(),
  syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
  keymap.of([
    ...defaultKeymap.filter((b) => !(b.mac && !b.key && /^Ctrl-\w$/.test(b.mac))),
    ...historyKeymap,
  ]),
];

// Inside the editor, Tab / Shift-Tab always adjust indentation regardless of
// cursor column — no tab character, no focus escape.
const tabIndent = Prec.highest(
  keymap.of([
    { key: "Tab", run: indentMore },
    { key: "Shift-Tab", run: indentLess },
  ]),
);

const darkTheme = EditorView.theme(
  {
    "&": { backgroundColor: "#1e1e1e", color: "#e5e7eb" },
    ".cm-content": { caretColor: "#e5e7eb" },
    ".cm-gutters": { backgroundColor: "#1e1e1e", color: "#6b7280", border: "none" },
    ".cm-activeLine": { backgroundColor: "#2a2a2a" },
    ".cm-activeLineGutter": { backgroundColor: "#2a2a2a" },
  },
  { dark: true },
);

export interface EditorHandle {
  getValue(): string;
  save(): void;
  focus(): void;
}

export type OnSave = (content: string) => void;

// Vim's `:w` maps to the same save action as the page's Save button.
Vim.defineEx("write", "w", (cm: { cm6: EditorView }) => {
  cm.cm6.dom.dispatchEvent(new CustomEvent("granite-save", { bubbles: true }));
});

/**
 * Mount a Vim-keybound markdown editor into `container`.
 * Calls `onSave(content)` when the user runs `:w` or invokes save() manually.
 */
export function init(
  container: HTMLElement,
  initialContent: string,
  onSave: OnSave,
): EditorHandle {
  const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  const view = new EditorView({
    doc: initialContent,
    extensions: [
      tabIndent,
      vim(),
      setup,
      markdown(),
      ...(prefersDark ? [darkTheme] : []),
    ],
    parent: container,
  });

  view.focus();

  container.addEventListener("granite-save", () => {
    onSave(view.state.doc.toString());
  });

  return {
    getValue: () => view.state.doc.toString(),
    save: () => onSave(view.state.doc.toString()),
    focus: () => view.focus(),
  };
}
