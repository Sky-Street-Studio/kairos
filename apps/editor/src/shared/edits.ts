import type { AssetEntry, CharacterFields } from "./types";

const INDENT_STEP = "    ";

const indentOf = (line: string) => line.length - line.trimStart().length;
const isBlank = (line: string) => {
  const t = line.trim();
  return t === "" || t.startsWith("#");
};

function splitLines(source: string): { lines: string[]; eol: string; trailing: boolean } {
  const eol = source.includes("\r\n") ? "\r\n" : "\n";
  const trailing = source.endsWith("\n");
  const lines = source.split(/\r?\n/);
  if (trailing) lines.pop();
  return { lines, eol, trailing };
}

function joinLines({ lines, eol, trailing }: { lines: string[]; eol: string; trailing: boolean }): string {
  return lines.join(eol) + (trailing || lines.length > 0 ? eol : "");
}

const labelHeader = (name: string) => new RegExp(`^\\s*label\\s+${name.replace(/[^\w]/g, "\\$&")}\\s*:\\s*(#.*)?$`);

/** Where a statement appended to the end of `label`'s body goes: a 0-based line index and its indentation. */
export function labelBodyEnd(lines: string[], label: string): { index: number; indent: string; last?: string } | null {
  const header = lines.findIndex((l) => labelHeader(label).test(l));
  if (header < 0) return null;
  const headerIndent = indentOf(lines[header]!);
  let bodyIndent: number | undefined;
  let last = header;
  for (let i = header + 1; i < lines.length; i++) {
    const line = lines[i]!;
    if (isBlank(line)) continue;
    if (indentOf(line) <= headerIndent) break;
    bodyIndent ??= indentOf(line);
    last = i;
  }
  const lastStatement = lines
    .slice(header + 1, last + 1)
    .toReversed()
    .find((l) => !isBlank(l) && indentOf(l) === bodyIndent);
  return {
    index: last + 1,
    indent: " ".repeat(bodyIndent ?? headerIndent + INDENT_STEP.length),
    last: lastStatement?.trim(),
  };
}

/** Appends `jump to` to the end of `from`'s body. Returns `null` when `from` is not in the source. */
export function insertJump(source: string, from: string, to: string): { source: string; changed: boolean } | null {
  const parts = splitLines(source);
  const end = labelBodyEnd(parts.lines, from);
  if (!end) return null;
  if (end.last === `jump ${to}`) return { source, changed: false };
  parts.lines.splice(end.index, 0, `${end.indent}jump ${to}`);
  return { source: joinLines(parts), changed: true };
}

const quote = (s: string) => `"${s.replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`;

export function formatCharacter({ id, name, color }: CharacterFields): string {
  return `character ${id} ${quote(name)}${color ? ` color=${quote(color)}` : ""}`;
}

export const CHARACTER_ID = /^[A-Za-z_]\w*$/;

export function validateCharacter(c: CharacterFields): string | null {
  if (!CHARACTER_ID.test(c.id)) return "Use letters, digits and underscores for the id, starting with a letter.";
  if (c.name.trim() === "") return "Give the character a display name.";
  if (c.color && !/^#[0-9a-fA-F]{6}$/.test(c.color)) return "Colors look like #c8ffc8.";
  return null;
}

/** Rewrites the `character` statement on 1-based `line`, keeping its indentation and trailing comment. */
export function setCharacterLine(source: string, line: number, character: CharacterFields): string {
  const parts = splitLines(source);
  const current = parts.lines[line - 1];
  if (current === undefined || !/^\s*character\s/.test(current)) {
    throw new Error(`line ${line} is not a character declaration`);
  }
  const comment = current.match(/\s+#[^"]*$/)?.[0] ?? "";
  parts.lines[line - 1] = " ".repeat(indentOf(current)) + formatCharacter(character) + comment;
  return joinLines(parts);
}

/** Adds a `character` statement after the last top-level one, or at the top of the file. */
export function addCharacter(source: string, character: CharacterFields): string {
  const parts = splitLines(source);
  let at = 0;
  parts.lines.forEach((l, i) => {
    if (/^character\s/.test(l)) at = i + 1;
  });
  parts.lines.splice(at, 0, formatCharacter(character));
  return joinLines(parts);
}

export type StagePosition = "left" | "center" | "right";

export function stagePosition(xFraction: number): StagePosition {
  if (xFraction < 1 / 3) return "left";
  if (xFraction < 2 / 3) return "center";
  return "right";
}

/** The statement a dropped asset turns into, or `null` for assets scripts cannot reference. */
export function statementForAsset(asset: AssetEntry, xFraction: number): string | null {
  if (asset.kind === "image") {
    return asset.name.split(/\s+/)[0] === "bg"
      ? `scene ${asset.name}`
      : `show ${asset.name} at ${stagePosition(xFraction)}`;
  }
  if (asset.kind === "audio") return `play music ${quote(asset.name)}`;
  return null;
}

/**
 * Where to put a new statement relative to 1-based `line`: on the next line, indented like that
 * line, or one level deeper when the line opens a block. Blank lines borrow from the line above.
 */
export function statementInsertion(lines: string[], line: number, statement: string): { after: number; text: string } {
  let anchor = Math.min(Math.max(line, 1), Math.max(lines.length, 1));
  while (anchor > 1 && isBlank(lines[anchor - 1] ?? "")) anchor--;
  const ref = lines[anchor - 1] ?? "";
  const code = ref.replace(/\s+#.*$/, "").trimEnd();
  const indent = " ".repeat(indentOf(ref)) + (code.endsWith(":") ? INDENT_STEP : "");
  return { after: isBlank(lines[line - 1] ?? "") ? line - 1 : line, text: indent + statement };
}

export function insertStatement(source: string, line: number, statement: string): { source: string; line: number } {
  const parts = splitLines(source);
  const { after, text } = statementInsertion(parts.lines, line, statement);
  parts.lines.splice(after, 0, text);
  return { source: joinLines(parts), line: after + 1 };
}
