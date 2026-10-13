import { FolderClock, PanelsTopLeft, Palette, Search } from "lucide-react";
import { useEffect, useRef, useState, type ComponentType } from "react";

import { Kbd } from "@/components/ui/input";
import { cn, modKey } from "@/lib/utils";

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
    keywords: "theme mode light dark system preset color gradient accent background tint import export json",
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
  const visible = PAGES.filter((p) => matches(p, query.trim().toLowerCase()));
  const page = visible.find((p) => p.id === pageId) ?? visible[0];
  const groups = [...new Set(visible.map((p) => p.group))];

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "f") {
        e.preventDefault();
        search.current?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <div className="@container flex min-w-0 flex-1">
      <div className="flex min-w-0 flex-1 flex-col @2xl:flex-row">
        <nav
          aria-label="Settings"
          className="flex shrink-0 flex-col border-b bg-sidebar/60 @2xl:w-56 @2xl:border-r @2xl:border-b-0"
        >
          <div className="border-b p-2.5">
            <label className="flex h-8 items-center gap-2 rounded-md border bg-background/60 px-2.5 text-muted-foreground focus-within:border-ring focus-within:shadow-[0_0_0_3px_var(--selection)]">
              <Search className="size-3.5 shrink-0" />
              <input
                ref={search}
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                onKeyDown={(e) => e.key === "Escape" && setQuery("")}
                placeholder="Search settings"
                aria-label="Search settings"
                className="min-w-0 flex-1 bg-transparent text-xs text-foreground outline-none placeholder:text-muted-foreground"
              />
              <Kbd>{modKey()}F</Kbd>
            </label>
          </div>
          <div className="flex gap-1 overflow-x-auto p-2.5 @2xl:block @2xl:flex-1 @2xl:overflow-y-auto">
            {groups.map((group) => (
              <div key={group} className="flex gap-1 @2xl:mb-4 @2xl:block">
                <h2 className="hidden px-2 pb-1.5 @2xl:block text-2xs font-semibold tracking-[0.08em] text-muted-foreground uppercase">
                  {group}
                </h2>
                {visible
                  .filter((p) => p.group === group)
                  .map((p) => (
                    <button
                      key={p.id}
                      type="button"
                      aria-current={p.id === page?.id ? "page" : undefined}
                      onClick={() => setPageId(p.id)}
                      className={cn(
                        "flex h-8 shrink-0 items-center gap-2.5 rounded-md px-2 text-[13px] whitespace-nowrap @2xl:w-full [&_svg]:size-4",
                        p.id === page?.id
                          ? "bg-accent font-medium text-foreground"
                          : "text-muted-foreground hover:bg-accent/60 hover:text-foreground",
                      )}
                    >
                      <p.icon />
                      {p.title}
                    </button>
                  ))}
              </div>
            ))}
            {visible.length === 0 && (
              <p className="px-2 text-xs text-muted-foreground">No settings match &ldquo;{query.trim()}&rdquo;.</p>
            )}
          </div>
        </nav>
        <div className="min-w-0 flex-1 overflow-y-auto">
          {page && (
            <div className="mx-auto max-w-3xl px-5 py-6 @2xl:px-8 @2xl:py-8">
              <header className="mb-6 border-b pb-5">
                <h1 className="text-xl font-semibold tracking-[-0.01em]">{page.title}</h1>
                <p className="mt-1 text-sm text-muted-foreground">{page.description}</p>
              </header>
              <div className="space-y-5">
                <page.Component />
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
