import { access, cp, mkdtemp, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

import { _electron as electron, expect, test } from "@playwright/test";

const editorDir = resolve(import.meta.dirname, "..");
const example = resolve(editorDir, "../../examples/hello-kairos");
const shots = process.env.KAIROS_SHOTS;

test("opens a project, shows the graph, reports diagnostics, stages assets and runs a terminal", async () => {
  const work = await mkdtemp(join(tmpdir(), "kairos-e2e-"));
  const project = join(work, "hello-kairos");
  await cp(example, project, { recursive: true });

  const app = await electron.launch({
    args: [editorDir, `--project=${project}`],
    env: { ...process.env, KAIROS_USER_DATA: join(work, "user-data") },
  });
  const page = await app.firstWindow();
  await page.setViewportSize({ width: 1480, height: 920 });

  await expect(page.getByText("Hello Kairos", { exact: true })).toBeVisible();
  await expect(page.getByTestId("diagnostic-count")).toHaveText("No problems");

  const playerBuilt = await access(resolve(editorDir, "../../crates/kairos-player/pkg/kairos_player.js")).then(
    () => true,
    () => false,
  );
  if (playerBuilt) {
    await expect(page.getByLabel("running")).toBeVisible({ timeout: 20_000 });
    await expect(page.getByRole("complementary", { name: "Preview" }).getByRole("alert")).toHaveCount(0);
    if (shots) await page.screenshot({ path: join(shots, "preview.png") });
  } else {
    await expect(page.getByTestId("player-missing")).toBeVisible();
  }

  await page.getByRole("tab", { name: /Story flow/ }).click();
  await expect(page.getByTestId("graph-node")).toHaveCount(4);
  await expect(page.getByTestId("graph-node").filter({ hasText: "meet_kai" })).toBeVisible();
  if (shots) await page.screenshot({ path: join(shots, "graph.png") });

  await page.getByRole("tab", { name: /script\.kai/ }).click();
  await page.locator(".monaco-editor .view-lines").click();
  const mod = process.platform === "darwin" ? "Meta" : "Control";
  await page.keyboard.press(process.platform === "darwin" ? "Meta+ArrowDown" : "Control+End");
  await page.keyboard.press("Enter");
  await page.keyboard.type("jump nowhere");
  await page.keyboard.press(`${mod}+s`);

  await expect(page.getByTestId("diagnostic-count")).toHaveText("1 problem");
  await expect(page.locator(".monaco-editor .squiggly-error")).toHaveCount(1);
  expect(await readFile(join(project, "script/script.kai"), "utf8")).toContain("jump nowhere");
  if (shots) await page.screenshot({ path: join(shots, "diagnostics.png") });

  const stage = page.getByTestId("preview-stage");
  const box = (await stage.boundingBox())!;
  await page
    .getByTestId("asset-thumb")
    .filter({ hasText: "e happy" })
    .dragTo(stage, { targetPosition: { x: box.width * 0.9, y: box.height / 2 } });
  await expect(page.locator(".monaco-editor .view-lines")).toContainText("show e happy at right");

  await page.keyboard.press(`${mod}+j`);
  await page.locator(".xterm").click();
  await page.keyboard.type("echo kairos-$((40+2))");
  await page.keyboard.press("Enter");
  await expect(page.locator(".xterm-rows")).toContainText("kairos-42", { timeout: 15_000 });

  await app.close();
});
