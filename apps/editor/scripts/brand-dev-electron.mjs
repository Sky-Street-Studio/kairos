// macOS takes the menu bar and Dock name from the bundle's Info.plist, which app.setName cannot
// change, so `bun run dev` would show "Electron". Packaged builds get the name from electron-builder.
import { execFileSync } from "node:child_process";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";

if (process.platform === "darwin") {
  const executable = createRequire(import.meta.url)("electron");
  const contents = dirname(dirname(executable));
  const plist = join(contents, "Info.plist");
  for (const key of ["CFBundleName", "CFBundleDisplayName"]) {
    execFileSync("plutil", ["-replace", key, "-string", "Kairos", plist]);
  }
  execFileSync("touch", [dirname(contents)]);
}
