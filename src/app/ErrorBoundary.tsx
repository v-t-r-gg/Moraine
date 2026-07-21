import { Component, type ErrorInfo, type ReactNode } from "react";

interface Props {
  children: ReactNode;
}

interface State {
  error: Error | null;
}

/** Top-level error boundary so init failures surface instead of a blank window. */
export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("[moraine] uncaught UI error", error, info.componentStack);
  }

  render() {
    if (this.state.error) {
      return (
        <div className="flex h-screen flex-col items-center justify-center gap-3 p-6 text-sm">
          <h1 className="text-lg font-semibold">Moraine failed to start</h1>
          <p style={{ color: "var(--muted)" }}>{this.state.error.message}</p>
          <button
            type="button"
            className="rounded border px-3 py-1"
            style={{ borderColor: "var(--border)" }}
            onClick={() => window.location.reload()}
          >
            Reload
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
