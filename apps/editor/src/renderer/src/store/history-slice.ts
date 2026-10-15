import type { HistoryEntry, HistorySlice, SliceCreator } from "./types";

const LIMIT = 200;
const BYTE_LIMIT = 32 * 1024 * 1024;
const TYPING_PAUSE_MS = 1000;
const OUTSIDE_CHANGE = "The script changed outside the editor, so undo history was cleared.";

const entryBytes = (entry: HistoryEntry) =>
  Object.values(entry.files).reduce((total, change) => total + 2 * (change.before.length + change.after.length), 0);

function trimHistory(past: HistoryEntry[], future: HistoryEntry[]) {
  let bytes = [...past, ...future].reduce((total, entry) => total + entryBytes(entry), 0);
  while (bytes > BYTE_LIMIT && (past.length > 0 || future.length > 0)) {
    if (future.length === 0 || (past.length > 0 && past[0]!.at <= future[0]!.at)) {
      bytes -= entryBytes(past[0]!);
      past = past.slice(1);
    } else {
      bytes -= entryBytes(future[0]!);
      future = future.slice(1);
    }
  }
  return { past, future };
}

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
          const history = trimHistory([...s.past.slice(0, -1), merged], []);
          return flags(history.past, history.future);
        }
        const history = trimHistory([...s.past, entry].slice(-LIMIT), []);
        return flags(history.past, history.future);
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
