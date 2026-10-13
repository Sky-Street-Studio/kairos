import { FileCode2, FileCog, FileImage, FileMusic, FileText } from "lucide-react";

import { fileKind } from "@shared/assets";

const ICONS = { script: FileCode2, text: FileCog, image: FileImage, audio: FileMusic };

export function FileIcon({ path, className }: { path: string; className?: string }) {
  const kind = fileKind(path);
  const Icon = kind ? ICONS[kind] : path.endsWith(".toml") ? FileCog : FileText;
  return <Icon className={className} />;
}
