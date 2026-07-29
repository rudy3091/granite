import { EditorView, minimalSetup } from "codemirror";
import { markdown } from "@codemirror/lang-markdown";
import { vim, Vim } from "@replit/codemirror-vim";

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
    extensions: [vim(), minimalSetup, markdown(), ...(prefersDark ? [darkTheme] : [])],
    parent: container,
  });

  container.addEventListener("granite-save", () => {
    onSave(view.state.doc.toString());
  });

  return {
    getValue: () => view.state.doc.toString(),
    save: () => onSave(view.state.doc.toString()),
  };
}
