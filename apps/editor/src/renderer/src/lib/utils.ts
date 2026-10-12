import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export const isMac = () => window.kairos.platform === "darwin";

export const modKey = () => (isMac() ? "⌘" : "Ctrl+");

export const basename = (path: string) => path.slice(path.lastIndexOf("/") + 1);
