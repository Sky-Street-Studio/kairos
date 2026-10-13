import { useEffect, useState } from "react";

import type { Scene } from "@shared/edit-types";

import { useApp } from "@/store";

interface LoadedScene {
  source: string;
  label: string;
  request: number;
  scene: Scene | null;
}

export function useScene(label: string) {
  const node = useApp((state) => state.analysis.graph.nodes.find((item) => item.label === label));
  const source = useApp((state) => (node ? state.buffers[node.path]?.source : undefined));
  const [loaded, setLoaded] = useState<LoadedScene | null>(null);
  const [request, setRequest] = useState(0);

  useEffect(() => {
    if (source === undefined) return;
    let live = true;
    void window.kairos.edit.blocks(source, label).then(
      (scene) => {
        if (live) setLoaded({ source, label, request, scene });
      },
      () => {
        if (live) setLoaded({ source, label, request, scene: null });
      },
    );
    return () => {
      live = false;
    };
  }, [source, label, request]);

  const scene = loaded?.label === label ? loaded.scene : null;
  return { path: node?.path ?? "", scene, refresh: () => setRequest((value) => value + 1) };
}
