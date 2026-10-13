import { cp, mkdtemp, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

import { _electron as electron, expect, test } from "@playwright/test";

const editorDir = resolve(import.meta.dirname, "..");
const example = resolve(editorDir, "../../examples/hello-kairos");

test("a beginner builds a branching scene without the code view", async () => {
  const work = await mkdtemp(join(tmpdir(), "kairos-visual-"));
  const project = join(work, "story");
  await cp(example, project, { recursive: true });
  const app = await electron.launch({
    args: [editorDir, `--project=${project}`],
    env: { ...process.env, KAIROS_USER_DATA: join(work, "data") },
  });
  const page = await app.firstWindow();
  await page.setViewportSize({ width: 1480, height: 920 });
  const mod = process.platform === "darwin" ? "Meta" : "Control";

  await page.getByRole("tab", { name: /Story flow/ }).click();
  await page.getByRole("button", { name: "Add scene" }).click();
  await page.getByLabel("Scene name").fill("Beach Day");
  await page.getByRole("button", { name: "Create" }).click();
  await expect(page.getByTestId("graph-node").filter({ hasText: "beach_day" })).toBeVisible();

  await page.getByTestId("graph-node").filter({ hasText: "about" }).dblclick();
  await expect(page.getByRole("tab", { name: "about" })).toHaveAttribute("aria-selected", "true");
  await expect(page.getByRole("listbox", { name: "Scene blocks" })).toBeVisible();
  await page.getByRole("button", { name: "Add block" }).last().click();
  await page.getByPlaceholder("Search blocks").fill("choice");
  await page.keyboard.press("Enter");
  await page.getByLabel("Option text").last().fill("Go to the beach");
  await page.waitForTimeout(400);
  await page.getByRole("button", { name: "Add block" }).last().click();
  await page.getByPlaceholder("Search blocks").fill("go to");
  await page.keyboard.press("Enter");
  await page.getByLabel("Scene", { exact: true }).selectOption("beach_day");
  await page.waitForTimeout(400);

  await page.getByRole("tab", { name: /script\.kai/ }).click();
  await page.locator(".monaco-editor .view-lines").click();
  await page.keyboard.press(process.platform === "darwin" ? "Meta+ArrowDown" : "Control+End");
  await expect(page.locator(".monaco-editor .view-lines")).toContainText('"Go to the beach":');
  await expect(page.locator(".monaco-editor .view-lines")).toContainText("jump beach_day");

  await page.keyboard.press(`${mod}+z`);
  await page.keyboard.press(`${mod}+z`);
  await page.keyboard.press(`${mod}+z`);
  await expect(page.locator(".monaco-editor .view-lines")).not.toContainText("jump beach_day");

  await page.keyboard.press(`${mod}+s`);
  expect(await readFile(join(project, "script/script.kai"), "utf8")).toContain("label beach_day:");
  await app.close();
});
