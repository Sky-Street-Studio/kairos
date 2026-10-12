import { toast } from "sonner";

import { appStore } from "@/store";

async function open(root: string) {
  try {
    await appStore().getState().openProject(root);
  } catch (e) {
    toast.error("Could not open project", { description: (e as Error).message });
  }
}

export async function openProjectDialog() {
  const root = await window.kairos.project.pick();
  if (root) await open(root);
}

export async function openRecent(root: string) {
  await open(root);
}

export async function newProjectFromTemplate() {
  try {
    const root = await window.kairos.project.createFromTemplate();
    if (!root) return;
    await open(root);
    toast.success("Project created", { description: root });
  } catch (e) {
    toast.error("Could not create project", { description: (e as Error).message });
  }
}
