import { stat } from "node:fs/promises";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

import { net, protocol } from "electron";

import { assetPathFromUrl } from "@shared/assets";
import { ASSET_SCHEME, PLAYER_SCHEME, type PlayerStatus } from "@shared/types";

import { playerPkgDir, within } from "./paths";
import { projectRoot } from "./project";

const privileges = { standard: true, secure: true, supportFetchAPI: true, corsEnabled: true, stream: true };

export function registerSchemes(): void {
  protocol.registerSchemesAsPrivileged([
    { scheme: ASSET_SCHEME, privileges },
    { scheme: PLAYER_SCHEME, privileges },
  ]);
}

const MIME: Record<string, string> = {
  js: "text/javascript",
  mjs: "text/javascript",
  wasm: "application/wasm",
  json: "application/json",
  png: "image/png",
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  webp: "image/webp",
  ogg: "audio/ogg",
  opus: "audio/ogg",
  mp3: "audio/mpeg",
  wav: "audio/wav",
  flac: "audio/flac",
  m4a: "audio/mp4",
};

const CORS = { "access-control-allow-origin": "*" };

async function serveFile(abs: string, request: Request): Promise<Response> {
  const res = await net.fetch(pathToFileURL(abs).toString(), { headers: request.headers });
  const headers = new Headers(res.headers);
  const type = MIME[abs.slice(abs.lastIndexOf(".") + 1).toLowerCase()];
  if (type) headers.set("content-type", type);
  for (const [k, v] of Object.entries(CORS)) headers.set(k, v);
  return new Response(res.body, { status: res.status, headers });
}

const notFound = (why: string) => new Response(why, { status: 404, headers: CORS });

export function handleProtocols(): void {
  protocol.handle(ASSET_SCHEME, async (request) => {
    const root = projectRoot();
    const rel = assetPathFromUrl(request.url);
    if (!root || !rel) return notFound("no such asset");
    try {
      return await serveFile(within(join(root, "assets"), rel), request);
    } catch {
      return notFound("no such asset");
    }
  });

  protocol.handle(PLAYER_SCHEME, async (request) => {
    const url = new URL(request.url);
    const file = decodeURIComponent(url.pathname.replace(/^\//, ""));
    try {
      return await serveFile(within(playerPkgDir(), file), request);
    } catch {
      return notFound("player package not built");
    }
  });
}

export async function playerStatus(): Promise<PlayerStatus> {
  const available = await stat(join(playerPkgDir(), "kairos_player.js")).then(
    () => true,
    () => false,
  );
  return {
    available,
    moduleUrl: `${PLAYER_SCHEME}://pkg/kairos_player.js`,
    buildHint: "scripts/build-wasm.sh",
  };
}
