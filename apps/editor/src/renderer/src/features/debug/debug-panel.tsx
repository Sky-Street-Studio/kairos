import { useState } from "react";

import { Button } from "@/components/ui/button";
import { useApp } from "@/store";

function VariableValue({ name, value, onCommit }: { name: string; value: unknown; onCommit(value: string): void }) {
  const [text, setText] = useState(JSON.stringify(value));
  return (
    <input
      aria-label={`Value of ${name}`}
      className="w-full rounded border bg-background px-1.5 py-1 font-mono focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
      value={text}
      onChange={(event) => setText(event.currentTarget.value)}
      onBlur={() => onCommit(text)}
    />
  );
}

function OverridesForm({
  state,
  onError,
}: {
  state: { path: string; line: number; variables: Record<string, unknown> };
  onError(error: string | null): void;
}) {
  const [overrides, setOverrides] = useState(JSON.stringify(state.variables, null, 2));
  const playFromLine = useApp((s) => s.playFromLine);
  const control = useApp((s) => s.debugControl);
  return (
    <form
      className="space-y-2"
      onSubmit={(event) => {
        event.preventDefault();
        try {
          const value: unknown = JSON.parse(overrides);
          if (!value || Array.isArray(value) || typeof value !== "object")
            throw new Error("Overrides must be a JSON object");
          playFromLine(state.path, state.line, value as Record<string, unknown>);
          onError(null);
        } catch (e) {
          onError(e instanceof Error ? e.message : String(e));
        }
      }}
    >
      <label htmlFor="debug-overrides" className="block font-medium">
        Play from here
      </label>
      <textarea
        id="debug-overrides"
        aria-label="Variable overrides"
        value={overrides}
        onChange={(event) => setOverrides(event.currentTarget.value)}
        spellCheck={false}
        className="h-24 w-full resize-y rounded-md border bg-background px-2 py-1.5 font-mono text-2xs focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
      />
      <Button size="sm" type="submit" disabled={!control}>
        Play from here
      </Button>
    </form>
  );
}

export function DebugPanel() {
  const state = useApp((s) => s.debugState);
  const control = useApp((s) => s.debugControl);
  const openFile = useApp((s) => s.openFile);
  const [error, setError] = useState<string | null>(null);

  if (!state) {
    return (
      <div className="flex h-full items-center px-3 text-xs text-muted-foreground">
        Start the preview to inspect its state.
      </div>
    );
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-1 gap-4 overflow-auto p-3 text-xs lg:grid-cols-2">
      <div className="min-w-0 space-y-4">
        <section aria-label="Variables">
          <h3 className="mb-2 font-medium">Variables</h3>
          <div className="max-h-44 overflow-auto rounded-md border">
            <table className="w-full text-left">
              <thead className="sticky top-0 bg-muted/70 text-muted-foreground">
                <tr>
                  <th className="px-2 py-1 font-medium">Name</th>
                  <th className="px-2 py-1 font-medium">Value</th>
                </tr>
              </thead>
              <tbody>
                {Object.entries(state.variables).map(([name, value]) => (
                  <tr key={name} className="border-t">
                    <th scope="row" className="px-2 py-1 font-mono font-normal">
                      {name}
                    </th>
                    <td className="px-2 py-1">
                      <VariableValue
                        key={`${name}:${JSON.stringify(value)}`}
                        name={name}
                        value={value}
                        onCommit={(input) => {
                          let json: string;
                          try {
                            JSON.parse(input);
                            json = input;
                          } catch {
                            json = JSON.stringify(input);
                          }
                          try {
                            control?.setVariable(name, json);
                            setError(null);
                          } catch (e) {
                            setError(e instanceof Error ? e.message : String(e));
                          }
                        }}
                      />
                    </td>
                  </tr>
                ))}
                {Object.keys(state.variables).length === 0 && (
                  <tr>
                    <td className="px-2 py-2 text-muted-foreground" colSpan={2}>
                      No variables
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        </section>

        <section aria-label="Call stack">
          <h3 className="mb-2 font-medium">Call stack</h3>
          <ol className="max-h-36 space-y-0.5 overflow-auto">
            {state.callStack.toReversed().map((frame, index) => (
              <li key={`${frame.path}:${frame.line}:${index}`}>
                <button
                  type="button"
                  disabled={!frame.path || frame.line < 1}
                  className="flex w-full items-center gap-2 rounded px-2 py-1 text-left hover:bg-accent focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-default disabled:opacity-50"
                  onClick={() => openFile(frame.path, frame.line)}
                >
                  <span className="min-w-0 flex-1 truncate font-medium">{frame.label}</span>
                  <span className="shrink-0 font-mono text-muted-foreground">
                    {frame.path}:{frame.line}
                  </span>
                </button>
              </li>
            ))}
          </ol>
        </section>
      </div>

      <div className="min-w-0 space-y-4">
        <section aria-label="History">
          <div className="mb-2 flex items-center justify-between">
            <h3 className="font-medium">History</h3>
            <span className="font-mono tabular-nums text-muted-foreground">
              {state.history === 0 ? "No history" : `${state.position + 1} / ${state.history}`}
            </span>
          </div>
          <input
            aria-label="History position"
            type="range"
            min={0}
            max={Math.max(0, state.history - 1)}
            value={state.position}
            disabled={!control || state.position === 0}
            onChange={(event) => {
              const position = Number(event.currentTarget.value);
              if (position < state.position) {
                try {
                  control?.rollbackTo(position);
                  setError(null);
                } catch (e) {
                  setError(e instanceof Error ? e.message : String(e));
                }
              }
            }}
            className="w-full accent-primary"
          />
        </section>

        <OverridesForm
          key={`${state.path}:${state.line}:${state.position}:${JSON.stringify(state.variables)}`}
          state={state}
          onError={setError}
        />
        {error && (
          <p role="alert" className="break-words text-destructive">
            {error}
          </p>
        )}
      </div>
    </div>
  );
}
