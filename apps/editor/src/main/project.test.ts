import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import sharp from "sharp";
import { afterEach, describe, expect, it } from "vitest";

import { importProjectAsset, loadProject, renameProjectAsset } from "./project";

let root: string | undefined;
let source: string | undefined;

afterEach(async () => {
  if (root) await rm(root, { recursive: true, force: true });
  if (source) await rm(source, { recursive: true, force: true });
  root = undefined;
  source = undefined;
});

async function project() {
  root = await mkdtemp(join(tmpdir(), "kairos-assets-project-"));
  await mkdir(join(root, "script"), { recursive: true });
  await mkdir(join(root, "assets/audio/music"), { recursive: true });
  await mkdir(join(root, "assets/images/sprites/e"), { recursive: true });
  await writeFile(join(root, "kairos.toml"), 'name = "Test"\n[window]\nwidth = 800\nheight = 600\n');
  await writeFile(join(root, "script/main.kai"), 'scene bg room\nshow e happy\nplay music "music/theme.ogg"\n');
  await loadProject(root);
  return root;
}

describe("project assets", () => {
  it("trims fully transparent edges from imported PNG sprites", async () => {
    const projectRoot = await project();
    source = await mkdtemp(join(tmpdir(), "kairos-assets-source-"));
    const png = join(source, "sprite.png");
    await sharp({ create: { width: 8, height: 6, channels: 4, background: { r: 0, g: 0, b: 0, alpha: 0 } } })
      .composite([
        {
          input: { create: { width: 2, height: 2, channels: 4, background: { r: 255, g: 0, b: 0, alpha: 1 } } },
          left: 3,
          top: 2,
        },
      ])
      .png()
      .toFile(png);

    const imported = await importProjectAsset(png, "assets/images/sprites/e", false, false);
    expect(imported.path).toBe("assets/images/sprites/e/sprite.png");
    const metadata = await sharp(join(projectRoot, imported.path!)).metadata();
    expect([metadata.width, metadata.height]).toEqual([2, 2]);
  });

  it("renames assets across script references and audio metadata keys", async () => {
    const projectRoot = await project();
    await writeFile(join(projectRoot, "assets/audio/music/theme.ogg"), "audio");
    await writeFile(join(projectRoot, "assets/audio/audio.toml"), '["music/theme.ogg"]\nloop_start = 2.5\n');

    await renameProjectAsset("assets/audio/music/theme.ogg", "title-theme");

    expect(await readFile(join(projectRoot, "script/main.kai"), "utf8")).toContain(
      'play music "music/title-theme.ogg"',
    );
    expect(await readFile(join(projectRoot, "assets/audio/audio.toml"), "utf8")).toContain('["music/title-theme.ogg"]');
    expect(await readFile(join(projectRoot, "assets/audio/music/title-theme.ogg"), "utf8")).toBe("audio");
  });

  it("updates a layered sprite default when its file is renamed", async () => {
    const projectRoot = await project();
    await mkdir(join(projectRoot, "assets/images/sprites/e/face"), { recursive: true });
    await writeFile(
      join(projectRoot, "assets/images/sprites/e/layers.toml"),
      'order = ["face"]\n[default]\nface = "happy"\n',
    );
    await writeFile(join(projectRoot, "assets/images/sprites/e/face/happy.png"), "image");

    await renameProjectAsset("assets/images/sprites/e/face/happy.png", "joyful");

    expect(await readFile(join(projectRoot, "assets/images/sprites/e/layers.toml"), "utf8")).toContain(
      'face = "joyful"',
    );
    expect(await readFile(join(projectRoot, "script/main.kai"), "utf8")).toContain("show e joyful");
    expect(await readFile(join(projectRoot, "assets/images/sprites/e/face/joyful.png"), "utf8")).toBe("image");
  });
});

describe("asset rename", () => {
  it("keeps a flat sprite's character tag when only the attribute is typed", async () => {
    const projectRoot = await project();
    await writeFile(join(projectRoot, "assets/images/sprites/e/e happy.png"), "image");

    await renameProjectAsset("assets/images/sprites/e/e happy.png", "joyful");

    expect(await readFile(join(projectRoot, "assets/images/sprites/e/e joyful.png"), "utf8")).toBe("image");
    expect(await readFile(join(projectRoot, "script/main.kai"), "utf8")).toContain("show e joyful\n");
  });

  it("updates title.toml images and escapes regex characters in names", async () => {
    const projectRoot = await project();
    await mkdir(join(projectRoot, "assets/images/ui"), { recursive: true });
    await mkdir(join(projectRoot, "ui"), { recursive: true });
    await writeFile(join(projectRoot, "assets/images/ui/logo (v1).png"), "image");
    await writeFile(join(projectRoot, "ui/title.toml"), 'logo = "logo (v1)"\nbackground = "bg room"\n');
    await writeFile(join(projectRoot, "script/main.kai"), "scene logo (v1)\nscene logo (v10)\n");

    await renameProjectAsset("assets/images/ui/logo (v1).png", "logo");

    const title = await readFile(join(projectRoot, "ui/title.toml"), "utf8");
    expect(title).toContain('logo = "logo"');
    expect(title).toContain('background = "bg room"');
    expect(await readFile(join(projectRoot, "script/main.kai"), "utf8")).toBe("scene logo\nscene logo (v10)\n");
  });
});

describe("loadProject translations", () => {
  it("includes only top-level TOML translations in the player snapshot", async () => {
    const dir = await mkdtemp(join(tmpdir(), "kairos-project-"));
    try {
      await writeFile(join(dir, "kairos.toml"), 'name = "Test"');
      await mkdir(join(dir, "tl/nested"), { recursive: true });
      await writeFile(join(dir, "tl/ja.toml"), '"id" = "Hola"');
      await writeFile(join(dir, "tl/notes.txt"), "ignored");
      await writeFile(join(dir, "tl/nested/fr.toml"), '"id" = "ignored"');

      const snapshot = await loadProject(dir);
      expect(snapshot.translations).toEqual([{ lang: "ja", source: '"id" = "Hola"' }]);
      expect(snapshot.tree.some((node) => node.path === "tl")).toBe(false);
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });
});
