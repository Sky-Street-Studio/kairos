import { toast } from "sonner";

export async function copyPath(root: string, path: string, relative = false): Promise<void> {
  const value = relative ? path : path ? `${root.replace(/[\\/]+$/, "")}/${path}` : root;
  try {
    await navigator.clipboard.writeText(value);
    toast.success(relative ? "Copied relative path" : "Copied path");
  } catch {
    toast.error("Could not copy path");
  }
}

export async function revealPath(path: string): Promise<void> {
  try {
    await window.kairos.shell.reveal(path);
  } catch (error) {
    toast.error("Could not reveal path", { description: error instanceof Error ? error.message : String(error) });
  }
}
