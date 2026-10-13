import type { HistoryEntry, HistorySlice, SliceCreator } from "./types";

const LIMIT = 200;
const TYPING_PAUSE_MS = 1000;
const OUTSIDE_CHANGE = "The script changed outside the editor, so undo history was cleared.";

export const createHistorySlice: SliceCreator<HistorySlice> = (set, get) => {
  const flags = (past: HistoryEntry[], future: HistoryEntry[]) => ({
    past,
    future,
    canUndo: past.length > 0,
    canRedo: future.length > 0,
  });
  const restore = (entry: HistoryEntry, side: "before" | "after") => {
    const expected = side === "before" ? "after" : "before";
    const current = get().buffers;
    if (Object.entries(entry.files).some(([path, change]) => current[path]?.source !== change[expected])) {
      set({ ...flags([], []), editError: OUTSIDE_CHANGE });
      return false;
    }
    set((s) => {
      const buffers = { ...s.buffers };
      for (const [path, change] of Object.entries(entry.files)) {
        const buffer = buffers[path];
        if (buffer) buffers[path] = { ...buffer, source: change[side] };
      }
      return { buffers };
    });
    get().scheduleAnalyze();
    return true;
  };

  return {
    past: [],
    future: [],
    canUndo: false,
    canRedo: false,
    record(entry, coalesce = false) {
      set((s) => {
        const last = s.past.at(-1);
        const paths = Object.keys(entry.files);
        const mergeable =
          coalesce &&
          last?.kind === "code" &&
          entry.at - last.at < TYPING_PAUSE_MS &&
          paths.length === 1 &&
          Object.keys(last.files).join() === paths[0];
        if (mergeable) {
          const path = paths[0]!;
          const merged = {
            ...last,
            at: entry.at,
            files: { [path]: { before: last.files[path]!.before, after: entry.files[path]!.after } },
          };
          return flags([...s.past.slice(0, -1), merged], []);
        }
        return flags([...s.past, entry].slice(-LIMIT), []);
      });
    },
    undo() {
      const entry = get().past.at(-1);
      if (!entry) return;
      if (!restore(entry, "before")) return;
      set((s) => flags(s.past.slice(0, -1), [...s.future, entry]));
    },
    redo() {
      const entry = get().future.at(-1);
      if (!entry) return;
      if (!restore(entry, "after")) return;
      set((s) => flags([...s.past, entry], s.future.slice(0, -1)));
    },
    clearHistory() {
      set(flags([], []));
    },
  };
};
