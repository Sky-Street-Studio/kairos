import { themeVariables } from "@kairos/theme";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal as XTerm } from "@xterm/xterm";
import { Bot, ChevronDown, Plus, SquareTerminal, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Tooltip } from "@/components/ui/tooltip";
import { useResolvedAppearance } from "@/hooks/use-theme";
import { cn } from "@/lib/utils";
import { DebugPanel } from "@/features/debug/debug-panel";
import { useApp } from "@/store";
import type { AgentInfo } from "@shared/types";

interface Session {
  key: number;
  title: string;
  agent?: AgentInfo["id"];
}

function useXtermTheme() {
  const theme = useApp((s) => s.theme);
  const appearance = useResolvedAppearance();
  const v = themeVariables(theme, appearance);
  return {
    background: "#00000000",
    foreground: v.foreground,
    cursor: v.primary,
    cursorAccent: v.card,
    selectionBackground: v.selection,
  };
}

function TerminalView({ session, visible, onExit }: { session: Session; visible: boolean; onExit(): void }) {
  const host = useRef<HTMLDivElement>(null);
  const term = useRef<XTerm | null>(null);
  const fit = useRef<FitAddon | null>(null);
  const theme = useXtermTheme();

  useEffect(() => {
    const xterm = new XTerm({
      fontFamily: '"Geist Mono Variable", ui-monospace, monospace',
      fontSize: 12.5,
      lineHeight: 1.25,
      cursorBlink: true,
      allowTransparency: true,
      scrollback: 5000,
      theme,
    });
    const fitAddon = new FitAddon();
    xterm.loadAddon(fitAddon);
    xterm.open(host.current!);
    term.current = xterm;
    fit.current = fitAddon;
    fitAddon.fit();

    let id: number | undefined;
    let disposed = false;
    const offData = window.kairos.terminal.onData((tid, data) => tid === id && xterm.write(data));
    const offExit = window.kairos.terminal.onExit((tid) => {
      if (tid === id) onExit();
    });
    void window.kairos.terminal.spawn({ cols: xterm.cols, rows: xterm.rows, agent: session.agent }).then((tid) => {
      if (disposed) window.kairos.terminal.kill(tid);
      else id = tid;
    });
    const input = xterm.onData((data) => id !== undefined && window.kairos.terminal.write(id, data));
    const resize = xterm.onResize(
      ({ cols, rows }) => id !== undefined && window.kairos.terminal.resize(id, cols, rows),
    );
    const observer = new ResizeObserver(() => {
      if (host.current?.offsetParent) fitAddon.fit();
    });
    observer.observe(host.current!);

    return () => {
      disposed = true;
      observer.disconnect();
      input.dispose();
      resize.dispose();
      offData();
      offExit();
      if (id !== undefined) window.kairos.terminal.kill(id);
      xterm.dispose();
    };
    // The session is fixed for this view's lifetime and theme changes are applied below.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (term.current) term.current.options.theme = theme;
  }, [theme]);

  useEffect(() => {
    if (visible) {
      requestAnimationFrame(() => {
        fit.current?.fit();
        term.current?.focus();
      });
    }
  }, [visible]);

  return <div ref={host} className={cn("absolute inset-0 py-1.5 pl-3", !visible && "invisible")} />;
}

let nextKey = 1;

export function BottomPanel({ open, height }: { open: boolean; height: number }) {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [active, setActive] = useState<number | null>(null);
  const [activeTab, setActiveTab] = useState<"debug" | "terminal">("terminal");
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const togglePanel = useApp((s) => s.togglePanel);
  const root = useApp((s) => s.project?.root);

  const add = (agent?: AgentInfo) => {
    const session = { key: nextKey++, title: agent?.name ?? "Terminal", agent: agent?.id };
    setSessions((s) => [...s, session]);
    setActive(session.key);
    setActiveTab("terminal");
    togglePanel("bottom", true);
  };

  const sessionsRef = useRef(sessions);
  sessionsRef.current = sessions;

  const close = (key: number) => {
    const next = sessionsRef.current.filter((x) => x.key !== key);
    setSessions(next);
    setActive((a) => (a === key ? (next.at(-1)?.key ?? null) : a));
    if (next.length === 0) togglePanel("bottom", false);
  };

  useEffect(() => {
    void window.kairos.terminal.agents().then(setAgents);
  }, []);

  useEffect(() => {
    if (open && sessions.length === 0) add();
    // Opening an empty panel starts one shell; `add` is stable enough for this.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  useEffect(() => {
    setSessions([]);
    setActive(null);
  }, [root]);

  if (sessions.length === 0 && !open) return null;

  return (
    <section
      style={{ height }}
      className={cn("flex shrink-0 flex-col overflow-hidden rounded-lg border bg-panel", !open && "hidden")}
      aria-label="Terminal"
    >
      <div className="flex h-8 shrink-0 items-center gap-0.5 border-b bg-background/25 pr-1.5 pl-1">
        <div
          role="tablist"
          aria-label="Bottom panel tabs"
          className="flex min-w-0 items-center gap-0.5 overflow-x-auto"
        >
          <button
            type="button"
            role="tab"
            aria-selected={activeTab === "debug"}
            onClick={() => setActiveTab("debug")}
            className={cn(
              "flex h-6 shrink-0 items-center rounded-md px-2 text-xs",
              activeTab === "debug" ? "bg-accent text-foreground" : "text-muted-foreground hover:text-foreground",
            )}
          >
            Debug
          </button>
          {sessions.map((s) => (
            <div
              key={s.key}
              role="tab"
              aria-selected={activeTab === "terminal" && s.key === active}
              onClick={() => {
                setActive(s.key);
                setActiveTab("terminal");
              }}
              className={cn(
                "group flex h-6 shrink-0 cursor-default items-center gap-1.5 rounded-md pr-1 pl-2 text-xs",
                s.key === active ? "bg-accent text-foreground" : "text-muted-foreground hover:text-foreground",
              )}
            >
              {s.agent ? <Bot className="size-3.5 text-primary" /> : <SquareTerminal className="size-3.5" />}
              {s.title}
              <button
                type="button"
                aria-label={`Close ${s.title}`}
                onClick={(e) => {
                  e.stopPropagation();
                  close(s.key);
                }}
                className="grid size-4 place-items-center rounded-sm opacity-0 group-hover:opacity-100 hover:bg-background/60"
              >
                <X className="size-3" />
              </button>
            </div>
          ))}
        </div>
        <Tooltip content="New terminal">
          <Button variant="ghost" size="icon" aria-label="New terminal" onClick={() => add()}>
            <Plus />
          </Button>
        </Tooltip>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="ghost" size="sm" className="ml-1">
              <Bot />
              Agent
              <ChevronDown className="!size-3" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start">
            <DropdownMenuLabel>Launch in the project folder</DropdownMenuLabel>
            {agents.map((a) => (
              <DropdownMenuItem key={a.id} disabled={!a.available} onSelect={() => add(a)}>
                <Bot />
                {a.name}
                <span className="ml-auto font-mono text-2xs text-muted-foreground">
                  {a.available ? a.id : "not on PATH"}
                </span>
              </DropdownMenuItem>
            ))}
          </DropdownMenuContent>
        </DropdownMenu>
        <Button
          variant="ghost"
          size="icon"
          aria-label="Hide panel"
          className="ml-auto"
          onClick={() => togglePanel("bottom", false)}
        >
          <X />
        </Button>
      </div>
      <div className="relative min-h-0 flex-1">
        {activeTab === "debug" ? (
          <DebugPanel />
        ) : (
          sessions.map((s) => (
            <TerminalView key={s.key} session={s} visible={open && s.key === active} onExit={() => close(s.key)} />
          ))
        )}
      </div>
    </section>
  );
}
