import { EditorView, minimalSetup } from "codemirror";
import { markdown } from "@codemirror/lang-markdown";
import { vim, Vim } from "@replit/codemirror-vim";

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
  const view = new EditorView({
    doc: initialContent,
    extensions: [vim(), minimalSetup, markdown()],
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
