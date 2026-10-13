import type { Place } from "@shared/edit-types";

export interface DropRow {
  line: number;
  endLine: number;
  depth: number;
  parentHeader: number;
}

export function dropPlace(rows: DropRow[], y: number, rects: DOMRect[]): Place | null {
  for (let i = 0; i < rows.length; i++) {
    const rect = rects[i];
    const row = rows[i];
    if (!rect || !row || y > rect.bottom) continue;
    if (y > rect.top + rect.height / 2) return { at: "after", line: row.line };
    const previous = rows.slice(0, i).findLast((candidate) => candidate.parentHeader === row.parentHeader);
    return previous ? { at: "after", line: previous.line } : { at: "start", header: row.parentHeader };
  }
  const last = rows.at(-1);
  return last ? { at: "after", line: last.line } : null;
}
