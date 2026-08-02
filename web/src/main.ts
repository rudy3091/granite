import "./style.css";
import { init, type EditorHandle } from "./editor";

interface NoteSummary {
  path: string;
  title: string;
  modified_ts: number;
}

interface NoteDetail {
  path: string;
  title: string | null;
  content: string;
}

const app = document.getElementById("app")!;
app.innerHTML = `
  <button id="sidebar-toggle" aria-label="Toggle note list"></button>
  <div id="sidebar">
    <h1>Granite</h1>
    <ul id="note-list"></ul>
  </div>
  <div id="main">
    <div id="toolbar">
      <button id="save-btn" disabled>Save</button>
      <span id="status"></span>
    </div>
    <div id="editor"></div>
  </div>
`;

const sidebar = document.getElementById("sidebar")!;
const sidebarToggle = document.getElementById("sidebar-toggle")!;
sidebarToggle.addEventListener("click", () => sidebar.classList.toggle("open"));

const noteList = document.getElementById("note-list")!;
const saveBtn = document.getElementById("save-btn") as HTMLButtonElement;
const status = document.getElementById("status")!;
const editorContainer = document.getElementById("editor")!;

let handle: EditorHandle | null = null;
let activePath: string | null = null;

// `path` from the API is rooted at the vault (`notes/...`); routes under
// `/api/notes/*path` expect it relative to `notes/` instead.
function relativePath(path: string): string {
  return path.replace(/^notes\//, "");
}

async function loadNotes(): Promise<void> {
  const notes: NoteSummary[] = await fetch("/api/notes").then((r) => r.json());
  notes.sort((a, b) => b.modified_ts - a.modified_ts);
  noteList.innerHTML = "";
  for (const note of notes) {
    const li = document.createElement("li");
    const a = document.createElement("a");
    a.textContent = note.title;
    a.href = "#";
    a.addEventListener("click", (e) => {
      e.preventDefault();
      openNote(relativePath(note.path));
    });
    li.appendChild(a);
    noteList.appendChild(li);
  }
  // The sidebar is hidden, so the most recently modified note is opened by
  // default — otherwise there's no way to get an editor onto the screen.
  if (notes.length > 0) {
    openNote(relativePath(notes[0].path));
  }
}

async function openNote(path: string): Promise<void> {
  const note: NoteDetail = await fetch(`/api/notes/${path}`).then((r) => r.json());
  activePath = path;
  editorContainer.innerHTML = "";
  handle = init(editorContainer, note.content, save);
  saveBtn.disabled = false;
  status.textContent = "";

  for (const a of noteList.querySelectorAll("a")) {
    a.classList.toggle("active", a.textContent === note.title);
  }
}

function save(content: string): void {
  if (!activePath) return;
  status.textContent = "Saving…";
  fetch(`/api/notes/${activePath}`, { method: "PUT", body: content })
    .then((r) => {
      status.textContent = r.ok ? "Saved" : "Save failed";
    })
    .catch(() => {
      status.textContent = "Save failed";
    });
}

saveBtn.addEventListener("click", () => handle?.save());

loadNotes();
