import { ArrowLeft, FolderClock, PanelsTopLeft, Palette, Search } from "lucide-react";
import { useEffect, useRef, useState, type ComponentType } from "react";

import { Kbd } from "@/components/ui/input";
import { cn, modKey } from "@/lib/utils";
import { useApp } from "@/store";

import { AppearancePage } from "./appearance-page";
import { LayoutPage } from "./layout-page";
import { ProjectsPage } from "./projects-page";

interface Page {
  id: string;
  group: string;
  title: string;
  description: string;
  keywords: string;
  icon: ComponentType<{ className?: string }>;
  Component: ComponentType;
}

const PAGES: Page[] = [
  {
    id: "appearance",
    group: "Interface",
    title: "Appearance",
    description: "Theme, light or dark mode, and the background and accent colors.",
    keywords: "theme mode light dark system preset color accent background tint import export json",
    icon: Palette,
    Component: AppearancePage,
  },
  {
    id: "layout",
    group: "Interface",
    title: "Layout",
    description: "Which panels show around the editor, and their sizes.",
    keywords: "panel sidebar preview terminal bottom size width height reset",
    icon: PanelsTopLeft,
    Component: LayoutPage,
  },
  {
    id: "projects",
    group: "Workspace",
    title: "Projects",
    description: "Projects you opened recently.",
    keywords: "recent open folder project",
    icon: FolderClock,
    Component: ProjectsPage,
  },
];

const matches = (page: Page, query: string) =>
  query.split(/\s+/).every((word) => `${page.title} ${page.description} ${page.keywords}`.toLowerCase().includes(word));

export function SettingsView() {
  const [pageId, setPageId] = useState(PAGES[0]!.id);
  const [query, setQuery] = useState("");
  const search = useRef<HTMLInputElement>(null);
  const toggleSettings = useApp((s) => s.toggleSettings);
  const visible = PAGES.filter((page) => matches(page, query.trim().toLowerCase()));
  const page = visible.find((item) => item.id === pageId) ?? visible[0];
  const groups = [...new Set(visible.map((item) => item.group))];

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "f") {
        event.preventDefault();
        search.current?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <div className="flex min-h-0 flex-1 bg-background">
      <nav aria-label="Settings" className="flex w-[300px] shrink-0 flex-col border-r bg-sidebar">
        <button
          type="button"
          onClick={() => toggleSettings(false)}
          className="flex h-12 shrink-0 items-center gap-2 border-b px-4 text-xs text-muted-foreground hover:bg-accent hover:text-foreground"
        >
          <ArrowLeft className="size-3.5" />
          Back to app
        </button>
        <label className="mx-3 my-3 flex h-8 shrink-0 items-center gap-2 rounded-md border bg-background px-2 text-muted-foreground focus-within:border-ring">
          <Search className="size-3.5" />
          <input
            ref={search}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={(event) => event.key === "Escape" && setQuery("")}
            placeholder="Search settings"
            aria-label="Search settings"
            className="min-w-0 flex-1 bg-transparent text-xs text-foreground outline-none placeholder:text-muted-foreground"
          />
          <Kbd>{modKey()}F</Kbd>
        </label>
        <div className="flex-1 overflow-y-auto px-2 pb-3">
          {groups.map((group) => (
            <div key={group} className="mb-4">
              <h2 className="px-2 pb-1.5 text-2xs font-semibold tracking-[0.08em] text-muted-foreground uppercase">
                {group}
              </h2>
              {visible
                .filter((item) => item.group === group)
                .map((item) => (
                  <button
                    key={item.id}
                    type="button"
                    aria-current={item.id === page?.id ? "page" : undefined}
                    onClick={() => setPageId(item.id)}
                    className={cn(
                      "flex h-8 w-full items-center gap-2.5 rounded-md px-2 text-[13px] [&_svg]:size-4",
                      item.id === page?.id
                        ? "bg-accent font-medium text-foreground"
                        : "text-muted-foreground hover:bg-accent/60 hover:text-foreground",
                    )}
                  >
                    <item.icon />
                    {item.title}
                  </button>
                ))}
            </div>
          ))}
          {visible.length === 0 && (
            <p className="px-2 text-xs text-muted-foreground">No settings match “{query.trim()}”.</p>
          )}
        </div>
      </nav>
      <main className="min-w-0 flex-1 overflow-y-auto">
        {page && (
          <div className="mx-auto max-w-[900px] px-8 py-8">
            <header className="mb-6 border-b pb-5">
              <h1 className="text-xl font-semibold tracking-[-0.01em]">{page.title}</h1>
              <p className="mt-1 text-sm text-muted-foreground">{page.description}</p>
            </header>
            <div className="space-y-5">
              <page.Component />
            </div>
          </div>
        )}
      </main>
    </div>
  );
}
