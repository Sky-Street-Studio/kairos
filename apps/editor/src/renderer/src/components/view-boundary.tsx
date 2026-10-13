import { TriangleAlert } from "lucide-react";
import { Component, type ReactNode } from "react";

import { Button } from "@/components/ui/button";

interface Props {
  /** Plain name of the area shown in the message, e.g. "Story flow". */
  name: string;
  children: ReactNode;
}

/**
 * Keeps one broken view from taking the whole window down: React unmounts everything above the
 * nearest boundary, so without one a single thrown render leaves a black screen.
 */
export class ViewBoundary extends Component<Props, { error: Error | null }> {
  state: { error: Error | null } = { error: null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  componentDidCatch(error: Error) {
    console.error(`[${this.props.name}]`, error);
  }

  render() {
    const { error } = this.state;
    if (!error) return this.props.children;
    return (
      <div role="alert" className="flex flex-1 flex-col items-center justify-center gap-3 p-6 text-center">
        <TriangleAlert className="size-5 text-destructive" />
        <div className="space-y-1">
          <p className="text-sm font-medium">{this.props.name} ran into a problem</p>
          <p className="max-w-md text-xs text-muted-foreground">
            The rest of the editor still works and your files are safe. Try again, or reopen the project if it keeps
            happening.
          </p>
        </div>
        <Button variant="secondary" size="md" onClick={() => this.setState({ error: null })}>
          Try again
        </Button>
        <details className="max-w-md text-left text-2xs text-muted-foreground">
          <summary className="cursor-default">Details</summary>
          <pre className="mt-1 whitespace-pre-wrap font-mono">{error.message}</pre>
        </details>
      </div>
    );
  }
}
