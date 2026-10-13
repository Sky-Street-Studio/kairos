import { mkdtemp, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

import { _electron as electron, expect, test } from "@playwright/test";

const editorDir = resolve(import.meta.dirname, "..");

test("a beginner creates a template project, follows the checklist and reveals code on request", async () => {
  const work = await mkdtemp(join(tmpdir(), "kairos-beginner-"));
  const app = await electron.launch({
    args: [editorDir],
    env: { ...process.env, KAIROS_USER_DATA: join(work, "data") },
  });
  const page = await app.firstWindow();
  await page.setViewportSize({ width: 1480, height: 920 });

  await page.getByRole("button", { name: "New project" }).click();
  await page.getByRole("radio", { name: /Branching romance/ }).click();
  await page.getByLabel("Project name").fill("first-story");
  await page.getByLabel("Project location").fill(work);
  await page.getByRole("button", { name: "Create project" }).click();

  await expect(page.getByTestId("beginner-checklist")).toBeVisible();
  await page.getByTestId("panel-files").click();
  await expect(page.getByTestId("file-tree-row").filter({ hasText: ".kai" })).toHaveCount(0);

  await page.keyboard.press(process.platform === "darwin" ? "Meta+," : "Control+,");
  await page.getByRole("button", { name: "Workspace" }).click();
  await expect(page.getByRole("switch", { name: "Show code" })).toHaveAttribute("aria-checked", "false");
  await page.getByRole("switch", { name: "Show code" }).click();
  await page.getByRole("button", { name: "Back to app" }).click();
  await page.getByTestId("panel-files").click();
  await page.getByTestId("file-tree-row").filter({ hasText: "script.kai" }).click();
  await expect(page.locator(".monaco-editor .view-lines")).toBeVisible();
  expect(await readFile(join(work, "first-story/script/script.kai"), "utf8")).toContain("affection += 1");
  await app.close();
});
