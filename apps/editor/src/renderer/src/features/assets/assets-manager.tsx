import { Boxes, FolderInput, Music2, Pencil, Play, Search, Square, Upload } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type DragEvent, type ReactNode } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useApp } from "@/store";
import { assetUsageCount, assetUrl, classifyAsset } from "@shared/assets";
import type { AssetCategory, AssetEntry, SourceFile } from "@shared/types";

import { startAssetDrag } from "../stage/asset-drag";

const CATEGORY_TITLES: Record<AssetCategory, string> = {
  background: "Backgrounds",
  sprite: "Sprites",
  ui: "UI",
  music: "Music",
  sound: "Sound",
  voice: "Voice",
  other: "Other",
};

function sourceFiles(buffers: Record<string, { path: string; source: string }>): SourceFile[] {
  return Object.values(buffers)
    .filter((file) => file.path.startsWith("script/") && file.path.endsWith(".kai"))
    .map(({ path, source }) => ({ path, source }));
}

function AssetRow({
  asset,
  usage,
  loopStart,
  playing,
  onPlay,
  onRename,
}: {
  asset: AssetEntry;
  usage: number;
  loopStart?: number;
  playing: boolean;
  onPlay(): void;
  onRename(): void;
}) {
  const image = asset.kind === "image";
  return (
    <fieldset
      draggable
      onDragStart={(event) => startAssetDrag(event, asset)}
      className="group m-0 flex min-h-12 min-w-0 items-center gap-3 border-0 border-b border-border/50 p-0 px-2 py-1.5 last:border-b-0 hover:bg-accent/35"
      data-testid="managed-asset"
    >
      {image ? (
        <div className="grid size-10 shrink-0 place-items-center overflow-hidden rounded border bg-background/70">
          <img
            src={assetUrl(asset.path)}
            alt=""
            loading="lazy"
            draggable={false}
            className="size-full object-contain"
          />
        </div>
      ) : (
        <div className="grid size-8 shrink-0 place-items-center rounded border bg-background/50 text-muted-foreground">
          <Music2 className="size-4" />
        </div>
      )}
      <div className="min-w-0 flex-1">
        <div className="truncate text-xs font-medium" title={asset.path}>
          {asset.name}
        </div>
        <div className="truncate text-2xs text-muted-foreground">{asset.path.replace(/^assets\//, "")}</div>
      </div>
      {asset.kind === "audio" && (
        <div className="flex shrink-0 items-center gap-2">
          {loopStart !== undefined && <span className="text-2xs text-muted-foreground">Loop {loopStart}s</span>}
          <Button
            variant="ghost"
            size="icon"
            aria-label={playing ? `Stop ${asset.name}` : `Play ${asset.name}`}
            onClick={onPlay}
          >
            {playing ? <Square /> : <Play />}
          </Button>
        </div>
      )}
      <span className="w-14 shrink-0 text-right text-2xs tabular-nums text-muted-foreground" title="Script references">
        {usage === 1 ? "1 use" : `${usage} uses`}
      </span>
      <Button variant="ghost" size="icon" aria-label={`Rename ${asset.name}`} onClick={onRename}>
        <Pencil />
      </Button>
    </fieldset>
  );
}

function ImportTarget({
  folder,
  onImport,
  onDrop,
  children,
}: {
  folder: string;
  onImport(folder: string): void;
  onDrop(event: DragEvent<HTMLElement>, folder: string): void;
  children: ReactNode;
}) {
  return (
    <section
      aria-label={`Drop files to import into ${folder}`}
      onDragOver={(event) => {
        if (event.dataTransfer.types.includes("Files")) event.preventDefault();
      }}
      onDrop={(event) => onDrop(event, folder)}
      className="rounded-md border border-dashed border-border/70 focus-within:border-ring"
    >
      {children}
      <button
        type="button"
        onClick={() => onImport(folder)}
        className="flex w-full items-center justify-center gap-1 border-t border-dashed border-border/60 py-1.5 text-2xs text-muted-foreground hover:bg-accent/40 hover:text-foreground focus-visible:outline-2 focus-visible:outline-ring"
      >
        <Upload className="size-3" /> Drop files here or choose files
      </button>
    </section>
  );
}

function Section({
  title,
  folder,
  assets,
  children,
  onImport,
  onDrop,
}: {
  title: string;
  folder: string;
  assets: AssetEntry[];
  children?: ReactNode;
  onImport(folder: string): void;
  onDrop(event: DragEvent<HTMLElement>, folder: string): void;
}) {
  return (
    <section className="border-b border-border/70 py-3 last:border-b-0">
      <div className="mb-2 flex items-center gap-2 px-1">
        <h2 className="text-sm font-semibold">{title}</h2>
        <span className="text-2xs tabular-nums text-muted-foreground">{assets.length}</span>
        <Button variant="ghost" size="sm" className="ml-auto" onClick={() => onImport(folder)}>
          <FolderInput /> Import
        </Button>
      </div>
      <ImportTarget folder={folder} onImport={onImport} onDrop={onDrop}>
        {children ?? (
          <p className="px-3 py-3 text-xs text-muted-foreground">
            No assets yet. Drop files here or import from a folder.
          </p>
        )}
      </ImportTarget>
    </section>
  );
}

export function AssetsManager() {
  const project = useApp((state) => state.project);
  const buffers = useApp((state) => state.buffers);
  const outlines = useApp((state) => state.analysis.outlines);
  const refreshTree = useApp((state) => state.refreshTree);
  const saveAll = useApp((state) => state.saveAll);
  const [filter, setFilter] = useState("");
  const [playingPath, setPlayingPath] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const characters = useMemo(
    () => [
      ...new Set(Object.values(outlines).flatMap((outline) => outline.characters.map((character) => character.id))),
    ],
    [outlines],
  );
  const scripts = useMemo(() => sourceFiles(buffers), [buffers]);
  const assets = useMemo(
    () =>
      (project?.assets ?? [])
        .filter((asset) => !asset.path.endsWith("/audio.toml") && !asset.path.endsWith("/layers.toml"))
        .map((asset) => classifyAsset(asset.path, characters)),
    [project, characters],
  );
  const visibleAssets = useMemo(() => {
    const query = filter.trim().toLowerCase();
    return query ? assets.filter((asset) => `${asset.name} ${asset.path}`.toLowerCase().includes(query)) : assets;
  }, [assets, filter]);
  const byCategory = (category: AssetCategory) => visibleAssets.filter((asset) => asset.category === category);

  useEffect(() => {
    if (!playingPath) return;
    const audio = new Audio(assetUrl(playingPath));
    audioRef.current = audio;
    const relativePath = playingPath.replace(/^assets\/audio\//, "");
    const loopStart = project?.audioMetadata?.[relativePath]?.loopStart;
    if (loopStart !== undefined) {
      audio.addEventListener("ended", () => {
        audio.currentTime = loopStart;
        void audio.play();
      });
    } else {
      audio.addEventListener("ended", () => setPlayingPath(null), { once: true });
    }
    void audio.play().catch(() => {
      setPlayingPath(null);
      setError(`Could not play ${relativePath}`);
    });
    return () => {
      audio.pause();
      audio.src = "";
      audioRef.current = null;
    };
  }, [playingPath, project]);

  const importFiles = async (files: File[], folder: string) => {
    setError(null);
    const media = files.filter((file) => Boolean((file as File & { path?: string }).path));
    if (!media.length) return;
    for (const file of media) {
      const image = /\.(png|jpe?g|webp)$/i.test(file.name);
      const convertWebp =
        image && window.confirm(`Import ${file.name} as WebP? Choose Cancel to keep its current format.`);
      const result = await window.kairos.project.importAsset(
        (file as File & { path: string }).path,
        folder,
        convertWebp,
      );
      if (!result) continue;
    }
    await refreshTree();
  };

  const onDrop = (event: DragEvent<HTMLElement>, folder: string) => {
    if (!event.dataTransfer.files.length) return;
    event.preventDefault();
    event.stopPropagation();
    void importFiles([...event.dataTransfer.files], folder).catch((cause: unknown) => setError(String(cause)));
  };

  const onImport = async (folder: string) => {
    setError(null);
    const canContainImages = !folder.startsWith("assets/audio/");
    const convertWebp =
      canContainImages &&
      window.confirm("Convert imported images to WebP? Choose Cancel to keep their current format.");
    try {
      if (await window.kairos.project.importAsset(null, folder, convertWebp)) await refreshTree();
    } catch (cause) {
      setError(String(cause));
    }
  };

  const rename = async (asset: AssetEntry) => {
    const currentName =
      asset.path
        .split("/")
        .at(-1)
        ?.replace(/\.[^.]+$/, "") ?? asset.name;
    const name = window.prompt("Rename asset", currentName);
    if (!name || name === currentName) return;
    try {
      await saveAll();
      await window.kairos.project.renameAsset(asset.path, name);
      await refreshTree();
    } catch (cause) {
      setError(String(cause));
    }
  };

  const row = (asset: AssetEntry) => {
    const relativeAudioPath = asset.path.replace(/^assets\/audio\//, "");
    return (
      <AssetRow
        key={asset.path}
        asset={asset}
        usage={assetUsageCount(asset, scripts, project?.spriteLayers)}
        loopStart={project?.audioMetadata?.[relativeAudioPath]?.loopStart}
        playing={playingPath === asset.path}
        onPlay={() => setPlayingPath(playingPath === asset.path ? null : asset.path)}
        onRename={() => void rename(asset)}
      />
    );
  };

  const spriteAssets = byCategory("sprite");
  const spriteIds = [
    ...new Set([
      ...characters,
      ...spriteAssets.map((asset) => asset.characterId).filter((id): id is string => Boolean(id)),
    ]),
  ].toSorted();
  const layeredGroups = project?.spriteLayers ?? {};
  const otherAssets = byCategory("other");
  const media = assets.filter((asset) => asset.kind !== "other");

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <header className="flex shrink-0 items-center gap-3 border-b px-4 py-3">
        <div className="grid size-8 place-items-center rounded-md border bg-muted/40 text-muted-foreground">
          <Boxes className="size-4" />
        </div>
        <div className="min-w-0 flex-1">
          <h1 className="text-sm font-semibold">Assets</h1>
          <p className="text-2xs text-muted-foreground">{media.length} images and audio files</p>
        </div>
        <div className="relative w-52">
          <Search className="pointer-events-none absolute top-1.5 left-2.5 size-3.5 text-muted-foreground" />
          <Input
            value={filter}
            onChange={(event) => setFilter(event.target.value)}
            placeholder="Find an asset"
            aria-label="Find an asset"
            className="pl-8"
          />
        </div>
      </header>
      {error && (
        <p role="alert" className="border-b border-destructive/30 bg-destructive/8 px-4 py-2 text-xs text-destructive">
          {error}
        </p>
      )}
      <div className="min-h-0 flex-1 overflow-y-auto px-4 pb-5">
        <Section
          title="Backgrounds"
          folder="assets/images/bg"
          assets={byCategory("background")}
          onImport={onImport}
          onDrop={onDrop}
        >
          {byCategory("background").length ? (
            <div className="grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))]">
              {byCategory("background").map(row)}
            </div>
          ) : undefined}
        </Section>
        <section className="border-b border-border/70 py-3">
          <div className="mb-2 flex items-center gap-2 px-1">
            <h2 className="text-sm font-semibold">Sprites</h2>
            <span className="text-2xs tabular-nums text-muted-foreground">{spriteAssets.length}</span>
            <Button variant="ghost" size="sm" className="ml-auto" onClick={() => onImport("assets/images/sprites")}>
              <FolderInput /> Import
            </Button>
          </div>
          <div className="space-y-3">
            {spriteIds.map((id) => {
              const characterAssets = spriteAssets.filter((asset) => asset.characterId === id);
              const layerData = layeredGroups[id];
              const groupNames = layerData?.order ?? [];
              const flatAssets = characterAssets.filter(
                (asset) => !asset.path.startsWith(`assets/images/sprites/${id}/`),
              );
              const groupedAssets = characterAssets.filter((asset) =>
                asset.path.startsWith(`assets/images/sprites/${id}/`),
              );
              return (
                <ImportTarget key={id} folder={`assets/images/sprites/${id}`} onImport={onImport} onDrop={onDrop}>
                  <div className="flex items-center gap-2 border-b px-2 py-1.5 text-xs font-medium">
                    <span className="size-2 rounded-full bg-primary/70" />
                    {id}
                  </div>
                  {groupNames.map((group) => {
                    const groupAssets = groupedAssets.filter((asset) =>
                      asset.path.startsWith(`assets/images/sprites/${id}/${group}/`),
                    );
                    const attributes = layerData?.groups[group] ?? [];
                    return (
                      <ImportTarget
                        key={group}
                        folder={`assets/images/sprites/${id}/${group}`}
                        onImport={onImport}
                        onDrop={onDrop}
                      >
                        <div className="flex items-center border-b bg-muted/20 px-3 py-1 text-2xs text-muted-foreground">
                          <span>
                            {group}
                            {attributes.length > 0 && <span className="ml-2">{attributes.join(" · ")}</span>}
                            {layerData?.defaults[group] && (
                              <span className="ml-2">default: {layerData.defaults[group]}</span>
                            )}
                          </span>
                        </div>
                        {groupAssets.map(row)}
                      </ImportTarget>
                    );
                  })}
                  {flatAssets.map(row)}
                  {groupedAssets
                    .filter(
                      (asset) =>
                        !groupNames.some((group) => asset.path.startsWith(`assets/images/sprites/${id}/${group}/`)),
                    )
                    .map(row)}
                  {!characterAssets.length && (
                    <p className="px-3 py-3 text-xs text-muted-foreground">No sprite files. Drop here to add one.</p>
                  )}
                  <button
                    type="button"
                    onClick={() => onImport(`assets/images/sprites/${id}`)}
                    className="flex w-full items-center justify-center gap-1 border-t border-dashed border-border/60 py-1.5 text-2xs text-muted-foreground hover:bg-accent/40"
                  >
                    <Upload className="size-3" /> Import for {id}
                  </button>
                </ImportTarget>
              );
            })}
            {spriteIds.length === 0 && (
              <ImportTarget folder="assets/images/sprites" onImport={onImport} onDrop={onDrop}>
                <p className="px-3 py-3 text-xs text-muted-foreground">
                  No character sprites. Declare a character in a script, then import its images here.
                </p>
              </ImportTarget>
            )}
          </div>
        </section>
        <Section title="UI" folder="assets/images/ui" assets={byCategory("ui")} onImport={onImport} onDrop={onDrop}>
          {byCategory("ui").map(row)}
        </Section>
        {(["music", "sound", "voice"] as const).map((category) => (
          <Section
            key={category}
            title={CATEGORY_TITLES[category]}
            folder={`assets/audio/${category === "sound" ? "sfx" : category}`}
            assets={byCategory(category)}
            onImport={onImport}
            onDrop={onDrop}
          >
            {byCategory(category).length ? byCategory(category).map(row) : undefined}
          </Section>
        ))}
        <Section title="Other" folder="assets" assets={otherAssets} onImport={onImport} onDrop={onDrop}>
          {otherAssets.length ? otherAssets.map(row) : undefined}
        </Section>
      </div>
    </div>
  );
}
