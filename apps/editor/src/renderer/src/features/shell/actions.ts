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

export function requestNewProject() {
  window.dispatchEvent(new Event("kairos:new-project"));
}

export async function createBeginnerProject(template: string, name: string, parent: string): Promise<boolean> {
  try {
    const root = await window.kairos.project.createFromTemplate(template, name, parent);
    if (!root) return false;
    await open(root);
    toast.success("Project created", { description: root });
    return true;
  } catch (e) {
    toast.error("Could not create project", { description: (e as Error).message });
    return false;
  }
}
