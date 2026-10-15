import { createServer } from "node:http";
import { access, cp, mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";

import { _electron as electron, expect, test } from "@playwright/test";

const editorDir = resolve(import.meta.dirname, "..");
const example = resolve(editorDir, "../../examples/hello-kairos");
const shots = process.env.KAIROS_SHOTS;

async function launch() {
  const work = await mkdtemp(join(tmpdir(), "kairos-e2e-"));
  const project = join(work, "hello-kairos");
  await cp(example, project, { recursive: true });

  const app = await electron.launch({
    args: [editorDir, `--project=${project}`],
    env: { ...process.env, KAIROS_USER_DATA: join(work, "user-data") },
  });
  const page = await app.firstWindow();
  await page.setViewportSize({ width: 1480, height: 920 });
  return { app, page, project };
}

test("opens a project, shows the graph, reports diagnostics, stages assets and runs a terminal", async () => {
  const { app, page, project } = await launch();
  const mod = process.platform === "darwin" ? "Meta" : "Control";

  await expect(page.getByText("Hello Kairos", { exact: true })).toBeVisible();
  await expect(page.getByTestId("diagnostic-count")).toHaveText("No problems");

  const playerBuilt = await access(resolve(editorDir, "../../crates/kairos-player/pkg/kairos_player.js")).then(
    () => true,
    () => false,
  );
  if (playerBuilt) {
    await expect(page.getByLabel("running")).toBeVisible({ timeout: 20_000 });
    const alerts = page.getByRole("complementary", { name: "Preview" }).getByRole("alert");
    await expect(alerts, `preview alert: ${(await alerts.allInnerTexts()).join(" | ")}`).toHaveCount(0);
    await page.keyboard.press(`${mod}+j`);
    await page.getByRole("tab", { name: "Debug" }).click();
    await expect(page.getByLabel("Value of curious")).toHaveValue("0");
    await page.getByRole("tab", { name: "Terminal" }).click();
    if (shots) await page.screenshot({ path: join(shots, "preview.png") });
  } else {
    await expect(page.getByTestId("player-missing")).toBeVisible();
  }

  await page.getByRole("tab", { name: /Story flow/ }).click();
  await expect(page.getByTestId("graph-node")).toHaveCount(5);
  await expect(page.getByTestId("graph-node").filter({ hasText: "meet_kai" })).toBeVisible();
  if (shots) await page.screenshot({ path: join(shots, "graph.png") });

  await page.getByRole("tab", { name: /script\.kai/ }).click();
  await page.locator(".monaco-editor .view-lines").click();
  await page.keyboard.press(process.platform === "darwin" ? "Meta+ArrowDown" : "Control+End");
  await page.keyboard.press("Enter");
  await page.keyboard.type("jump nowhere");
  await page.keyboard.press(`${mod}+s`);

  await expect(page.getByTestId("diagnostic-count")).toHaveText("1 problem");
  await expect(page.locator(".monaco-editor .squiggly-error")).toHaveCount(1);
  expect(await readFile(join(project, "script/script.kai"), "utf8")).toContain("jump nowhere");
  if (shots) await page.screenshot({ path: join(shots, "diagnostics.png") });

  await page.getByRole("button", { name: "Open asset manager" }).click();
  await expect(page.getByRole("heading", { name: "Backgrounds" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Sprites" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Music" })).toBeVisible();
  await expect(page.getByTestId("managed-asset").filter({ hasText: "e happy" })).toBeVisible();

  const stage = page.getByTestId("preview-stage");
  const box = (await stage.boundingBox())!;
  await page
    .getByTestId("asset-thumb")
    .filter({ hasText: "e happy" })
    .dragTo(stage, { targetPosition: { x: box.width * 0.9, y: box.height / 2 } });
  await expect(page.locator(".monaco-editor .view-lines")).toContainText("show e happy at right");

  if (!playerBuilt) {
    await page.keyboard.press(`${mod}+j`);
    await expect(page.getByRole("tab", { name: "Debug" })).toBeVisible();
    await page.getByRole("tab", { name: "Debug" }).click();
    await page.getByRole("tab", { name: "Terminal" }).click();
  }
  await page.locator(".xterm").click();
  await page.keyboard.type("echo kairos-$((40+2))");
  await page.keyboard.press("Enter");
  await expect(page.locator(".xterm-rows")).toContainText("kairos-42", { timeout: 15_000 });

  await app.close();
});

test("prevents dropped HTML files from navigating the application window", async () => {
  const { app, page } = await launch();
  await expect(page.getByText("Hello Kairos", { exact: true })).toBeVisible();
  const result = await page.evaluate(() => {
    type Browser = {
      DataTransfer: new () => { items: { add(file: unknown): void } };
      File: new (parts: string[], name: string, options: { type: string }) => unknown;
      DragEvent: new (
        type: string,
        options: { bubbles: boolean; cancelable: boolean; dataTransfer: unknown },
      ) => { defaultPrevented: boolean };
      document: { dispatchEvent(event: unknown): void };
    };
    const browser = globalThis as unknown as Browser;
    const data = new browser.DataTransfer();
    data.items.add(new browser.File(["<script>window.kairos = undefined</script>"], "dropped.html", { type: "text/html" }));
    const event = new browser.DragEvent("drop", { bubbles: true, cancelable: true, dataTransfer: data });
    browser.document.dispatchEvent(event);
    return { prevented: event.defaultPrevented };
  });
  expect(result.prevented).toBe(true);
  await expect(page.getByText("Hello Kairos", { exact: true })).toBeVisible();
  await app.close();
});

test("blocks redirects away from the trusted renderer", async () => {
  const { app, page } = await launch();
  await expect(page.getByText("Hello Kairos", { exact: true })).toBeVisible();
  let redirectRequested = false;
  const server = createServer((_request, response) => {
    redirectRequested = true;
    response.writeHead(302, { Location: "https://example.com/" }).end();
  });
  await new Promise<void>((finish, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", finish);
  });
  const address = server.address();
  if (!address || typeof address === "string") throw new Error("Redirect test server did not bind a port");

  try {
    await app.evaluate(async ({ BrowserWindow }, url) => {
      await BrowserWindow.getAllWindows()[0]!.webContents.loadURL(url).catch(() => undefined);
    }, `http://127.0.0.1:${address.port}/redirect`);
    const finalUrl = await app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.webContents.getURL());
    expect(redirectRequested).toBe(true);
    expect(finalUrl).not.toBe("https://example.com/");
  } finally {
    await app.close();
    await new Promise<void>((finish) => server.close(() => finish()));
  }
});

test("rejects renderer-supplied project, asset and export paths", async () => {
  const { app, page, project } = await launch();
  const results = await page.evaluate(async (outsidePath) => {
    type Api = {
      kairos: {
        project: {
          open(path: string): Promise<unknown>;
          importAsset(path: string, folder: string, convertWebp?: boolean): Promise<unknown>;
          export(target: "web", output: string, packAssets: boolean): Promise<unknown>;
        };
      };
    };
    const api = (globalThis as unknown as Api).kairos;
    const rejected = async (operation: () => Promise<unknown>) => {
      try {
        await operation();
        return false;
      } catch (error) {
        return error instanceof Error && error.message.includes("Choose this path");
      }
    };
    return Promise.all([
      rejected(() => api.project.open(outsidePath)),
      rejected(() => api.project.importAsset(outsidePath, "assets")),
      rejected(() => api.project.export("web", outsidePath, false)),
    ]);
  }, dirname(project));
  expect(results).toEqual([true, true, true]);
  await app.close();
});

test("copies an absolute path from the Files tree context menu", async () => {
  const { app, page, project } = await launch();
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.getByTestId("panel-files").click();
  const filter = page.getByLabel("Find files");
  await filter.fill("label start");
  await expect(page.getByText(/No files match/)).toBeVisible();
  await filter.fill("script.kai");
  const row = page.locator('[data-testid="file-tree-row"]').filter({ hasText: "script.kai" });
  await row.click({ button: "right" });
  await page.getByRole("menuitem", { name: "Copy path" }).click();
  await expect
    .poll(() =>
      page.evaluate(() =>
        (navigator as Navigator & { clipboard: { readText(): Promise<string> } }).clipboard.readText(),
      ),
    )
    .toBe(join(project, "script/script.kai"));
  const revealError = await page.evaluate(async (outsidePath) => {
    const api = (globalThis as unknown as { kairos: { shell: { reveal(path: string): Promise<void> } } }).kairos;
    try {
      await api.shell.reveal(outsidePath);
      return "Path was accepted";
    } catch (error) {
      return error instanceof Error ? error.message : String(error);
    }
  }, dirname(project));
  expect(revealError).toContain("outside the project");
  await app.close();
});

test("opens a scene from the button on its card", async () => {
  const { app, page } = await launch();
  await page.getByRole("tab", { name: /Story flow/ }).click();
  const node = page.getByTestId("graph-node").filter({ hasText: /^about/ });
  await node.hover();
  await page.getByRole("button", { name: "Open about" }).click();
  await expect(page.getByRole("tab", { name: /about/ })).toHaveAttribute("aria-selected", "true");
  await app.close();
});

test("ignores renderer attempts to add recent project paths", async () => {
  const { app, page, project } = await launch();
  await expect(page.getByText("Hello Kairos", { exact: true })).toBeVisible();
  const result = await page.evaluate(async (unapprovedPath) => {
    const api = (globalThis as unknown as {
      kairos: {
        settings: {
          get(): Promise<{ recentProjects: string[] }>;
          update(patch: { recentProjects: string[] }): Promise<unknown>;
        };
      };
    }).kairos;
    const before = await api.settings.get();
    await api.settings.update({ recentProjects: [...before.recentProjects, unapprovedPath] });
    return { before: before.recentProjects, after: (await api.settings.get()).recentProjects };
  }, dirname(project));
  expect(result.before).toContain(project);
  expect(result.after).toEqual(result.before);
  await app.close();
});

test("starts on the project picker even when there is a recent project", async () => {
  const work = await mkdtemp(join(tmpdir(), "kairos-e2e-"));
  const project = join(work, "hello-kairos");
  await cp(example, project, { recursive: true });
  const userData = join(work, "user-data");
  await mkdir(userData, { recursive: true });
  await writeFile(join(userData, "settings.json"), JSON.stringify({ recentProjects: [project] }));

  const app = await electron.launch({ args: [editorDir], env: { ...process.env, KAIROS_USER_DATA: userData } });
  const page = await app.firstWindow();
  await expect(page.getByText("Write your visual novel")).toBeVisible();
  await expect(page.getByRole("button", { name: /hello-kairos/ })).toBeVisible();
  await app.close();
});

test("brings hidden panels back from the tab strip and the application menu", async () => {
  const { app, page } = await launch();
  const mod = process.platform === "darwin" ? "Meta" : "Control";
  const sidebar = page.getByRole("button", { name: "Toggle sidebar" });
  await expect(sidebar).toBeVisible();

  await sidebar.click();
  await expect(sidebar).toBeHidden();
  if (shots) await page.screenshot({ path: join(shots, "sidebar-hidden.png") });
  await page.getByRole("button", { name: "Show sidebar" }).click();
  await expect(sidebar).toBeVisible();

  await page.keyboard.press(`${mod}+\\`);
  await page.getByRole("button", { name: "Show preview" }).click();
  await expect(page.getByRole("button", { name: "Show preview" })).toBeHidden();

  await app.evaluate(({ BrowserWindow, Menu }) => {
    const win = BrowserWindow.getAllWindows()[0]!;
    const item = Menu.getApplicationMenu()!
      .items.flatMap((menu) => menu.submenu?.items ?? [])
      .find((i) => i.label === "Toggle Sidebar")!;
    item.click(undefined, win, undefined);
  });
  await expect(sidebar).toBeHidden();
  await app.close();
});

test("previews images, audio and the manifest, and navigates settings", async () => {
  const { app, page } = await launch();
  await expect(page.getByText("Hello Kairos", { exact: true })).toBeVisible();
  await page.getByTestId("panel-files").click();

  await page.getByRole("button", { name: "bg room.png", exact: true }).click();
  await expect(page.getByRole("tab", { name: /bg room\.png/ })).toHaveAttribute("aria-selected", "true");
  const image = page.getByTestId("image-preview");
  await expect(image).toBeVisible();
  await expect.poll(() => image.evaluate((img) => (img as { naturalWidth: number }).naturalWidth)).toBeGreaterThan(0);
  if (shots) await page.screenshot({ path: join(shots, "image-preview.png") });
  await page.getByRole("button", { name: "Show in Assets" }).click();
  await expect(page.getByTestId("managed-asset").filter({ hasText: "bg room" })).toHaveAttribute(
    "aria-current",
    "true",
  );
  if (shots) await page.screenshot({ path: join(shots, "show-in-assets.png") });

  await page.getByRole("button", { name: "theme.ogg", exact: true }).click();
  const audio = page.getByTestId("audio-preview");
  await expect.poll(() => audio.evaluate((el) => (el as { readyState: number }).readyState)).toBeGreaterThanOrEqual(1);
  if (shots) await page.screenshot({ path: join(shots, "audio-preview.png") });

  await page.getByRole("button", { name: "kairos.toml", exact: true }).click();
  await expect(page.getByTestId("text-preview")).toContainText('name = "Hello Kairos"');
  if (shots) await page.screenshot({ path: join(shots, "text-preview.png") });
  await page.getByRole("button", { name: "audio.toml", exact: true }).click();
  await expect(page.getByTestId("text-preview")).toContainText("loop_start");
  if (shots) await page.screenshot({ path: join(shots, "audio-toml.png") });

  await page.getByRole("tab", { name: /script\.kai/ }).click();
  await expect(page.locator("[data-testid=monaco-host] .view-lines")).toContainText("label start");

  await page.keyboard.press(process.platform === "darwin" ? "Meta+," : "Control+,");
  await expect(page.getByRole("heading", { name: "Appearance", level: 1 })).toBeVisible();
  if (shots) await page.screenshot({ path: join(shots, "settings-appearance.png") });
  await page.getByRole("navigation", { name: "Settings" }).getByRole("button", { name: "Layout" }).click();
  const sidebarSwitch = page.getByRole("switch", { name: "Show sidebar" });
  await expect(sidebarSwitch).toHaveAttribute("aria-checked", "true");
  if (shots) await page.screenshot({ path: join(shots, "settings-layout.png") });
  await page.getByLabel("Search settings").fill("recent");
  await expect(page.getByRole("heading", { name: "Projects", level: 1 })).toBeVisible();
  if (shots) await page.screenshot({ path: join(shots, "settings.png") });
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("panel-files")).toBeVisible();
  await app.close();
});
