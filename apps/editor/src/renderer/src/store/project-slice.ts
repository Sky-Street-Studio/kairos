import type { ProjectSnapshot } from "@shared/types";

import type { FileBuffer, ProjectSlice, SliceCreator } from "./types";

const withoutScripts = ({ scripts: _scripts, ...rest }: ProjectSnapshot) => rest;

const isBlank = (line: string) => /^\s*(#.*)?$/.test(line);

/**
 * Puts `statement` after 1-based `line` (default: the last line), indented like the nearest non-blank
 * line at or above it, one level deeper when that line opens a block. A blank target line takes the
 * statement in its place so the blank line stays below it.
 */
function insertStatement(source: string, line: number | undefined, statement: string) {
  const eol = source.includes("\r\n") ? "\r\n" : "\n";
  const trailing = source.endsWith("\n");
  const lines = source.split(/\r?\n/);
  if (trailing) lines.pop();
  const target = Math.min(Math.max(line ?? lines.length, 1), Math.max(lines.length, 1));
  let anchor = target;
  while (anchor > 1 && isBlank(lines[anchor - 1] ?? "")) anchor--;
  const ref = lines[anchor - 1] ?? "";
  const code = ref.replace(/\s+#.*$/, "").trimEnd();
  const indent = (ref.match(/^\s*/)?.[0] ?? "") + (code.endsWith(":") ? "    " : "");
  const after = isBlank(lines[target - 1] ?? "") ? target - 1 : target;
  lines.splice(after, 0, indent + statement);
  return { source: lines.join(eol) + (trailing || lines.length > 0 ? eol : ""), line: after + 1 };
}

export const isDirty = (b: FileBuffer | undefined) => !!b && b.source !== b.saved;

export const createProjectSlice: SliceCreator<ProjectSlice> = (set, get, api) => ({
  project: null,
  buffers: {},
  loading: false,
  checklistDismissed: false,
  beginnerPath: false,

  async openProject(root) {
    set({ loading: true });
    try {
      const snap = await api.project.open(root);
      const buffers = Object.fromEntries(snap.scripts.map((f) => [f.path, { ...f, saved: f.source }]));
      const first = snap.scripts[0]?.path;
      const settings = await api.settings.get();
      set({
        project: withoutScripts(snap),
        showCode: !settings.hiddenCodeProjects.includes(root),
        checklistDismissed: settings.dismissedChecklistProjects.includes(root),
        beginnerPath: settings.beginnerProjects.includes(root),
        buffers,
        tabs: [
          { id: "graph", kind: "graph" },
          ...(first && !settings.hiddenCodeProjects.includes(root)
            ? [{ id: first, kind: "file" as const, path: first }]
            : []),
        ],
        activeId: first && !settings.hiddenCodeProjects.includes(root) ? first : "graph",
        cursor: null,
        reveal: null,
        playRequest: null,
        hasPlayed: false,
      });
      get().clearHistory();
      await get().loadGraphLayout();
      await get().analyze();
    } finally {
      set({ loading: false });
    }
  },

  async refreshTree() {
    const snap = await api.project.reload();
    if (!snap) return;
    set((s) => {
      const buffers = Object.fromEntries(
        snap.scripts.map((f) => {
          const current = s.buffers[f.path];
          return [f.path, current && isDirty(current) ? current : { ...f, saved: f.source }];
        }),
      );
      for (const [path, buffer] of Object.entries(s.buffers)) {
        if (!(path in buffers) && isDirty(buffer)) buffers[path] = buffer;
      }
      return { project: withoutScripts(snap), buffers };
    });
    await get().analyze();
  },

  setSource(path, source) {
    const current = get().buffers[path];
    if (!current || current.source === source) return;
    set((s) => ({ buffers: { ...s.buffers, [path]: { ...current, source } } }));
    get().record(
      {
        files: { [path]: { before: current.source, after: source } },
        kind: "code",
        at: Date.now(),
      },
      true,
    );
    get().scheduleAnalyze();
  },

  async save(path) {
    const buffer = get().buffers[path];
    if (!buffer) return;
    await api.project.writeFile(path, buffer.source);
    set((s) => ({
      buffers: { ...s.buffers, [path]: { ...buffer, saved: buffer.source } },
      previewVersion: s.previewVersion + 1,
    }));
    await get().analyze();
  },

  async saveAll() {
    const dirty = Object.values(get().buffers).filter(isDirty);
    for (const b of dirty) await get().save(b.path);
  },

  insertAtCursor(statement, line) {
    const { cursor, activeId, tabs, buffers } = get();
    const active = tabs.find((t) => t.id === activeId);
    const path = active?.kind === "file" && active.path in buffers ? active.path : cursor?.path;
    const buffer = path ? buffers[path] : undefined;
    if (!path || !buffer) return false;
    const next = insertStatement(buffer.source, line ?? (cursor?.path === path ? cursor.line : undefined), statement);
    get().setSource(path, next.source);
    get().openFile(path, next.line);
    return true;
  },
});
