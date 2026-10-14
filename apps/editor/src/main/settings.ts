import { access, mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";

import { DEFAULT_THEME, parseThemeJson } from "@kairos/theme";
import { app } from "electron";

import type { LayoutPrefs, Settings } from "@shared/types";

export const DEFAULT_LAYOUT: LayoutPrefs = {
  sidebar: true,
  preview: true,
  bottom: false,
  sidebarWidth: 260,
  previewWidth: 400,
  bottomHeight: 240,
};

const file = () => join(app.getPath("userData"), "settings.json");
/** Where settings lived before the app was named Kairos; read once so recent projects survive the move. */
const legacyFile = () => join(app.getPath("appData"), "@kairos", "editor", "settings.json");

let cached: Settings | undefined;

function sanitize(raw: unknown): Settings {
  const r = (typeof raw === "object" && raw !== null ? raw : {}) as Partial<Settings>;
  const theme = parseThemeJson(JSON.stringify(r.theme));
  return {
    theme: theme.ok ? theme.theme : DEFAULT_THEME,
    recentProjects: Array.isArray(r.recentProjects)
      ? r.recentProjects.filter((p): p is string => typeof p === "string").slice(0, 10)
      : [],
    layout: { ...DEFAULT_LAYOUT, ...(typeof r.layout === "object" ? r.layout : {}) },
    hiddenCodeProjects: Array.isArray(r.hiddenCodeProjects)
      ? r.hiddenCodeProjects.filter((p): p is string => typeof p === "string")
      : [],
    beginnerProjects: Array.isArray(r.beginnerProjects)
      ? r.beginnerProjects.filter((p): p is string => typeof p === "string")
      : [],
    dismissedChecklistProjects: Array.isArray(r.dismissedChecklistProjects)
      ? r.dismissedChecklistProjects.filter((p): p is string => typeof p === "string")
      : [],
    reopenLastProject: r.reopenLastProject === true,
  };
}

export async function getSettings(): Promise<Settings> {
  if (cached) return cached;
  const raw = await readFile(file(), "utf8")
    .catch((err) => (process.env.KAIROS_USER_DATA ? Promise.reject(err) : readFile(legacyFile(), "utf8")))
    .then(JSON.parse, () => ({}));
  const loaded = sanitize(raw);
  const existing = await existingProjects(loaded.recentProjects);
  cached = loaded;
  if (existing.length !== loaded.recentProjects.length) await updateSettings({ recentProjects: existing });
  return cached;
}

async function existingProjects(roots: string[]): Promise<string[]> {
  const found = await Promise.all(
    roots.map((root) =>
      access(join(root, "kairos.toml")).then(
        () => true,
        () => false,
      ),
    ),
  );
  return roots.filter((_, i) => found[i]);
}

export async function updateSettings(patch: Partial<Settings>): Promise<Settings> {
  cached = sanitize({ ...(await getSettings()), ...patch });
  await mkdir(dirname(file()), { recursive: true });
  await writeFile(file(), JSON.stringify(cached, null, 2), "utf8");
  return cached;
}

export async function rememberProject(root: string): Promise<void> {
  const { recentProjects } = await getSettings();
  await updateSettings({ recentProjects: [root, ...recentProjects.filter((p) => p !== root)] });
}
