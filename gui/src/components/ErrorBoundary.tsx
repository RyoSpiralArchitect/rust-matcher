import { Component } from "react";
import type { ErrorInfo, ReactNode } from "react";

interface ErrorBoundaryProps {
  children: ReactNode;
}

interface ErrorBoundaryState {
  hasError: boolean;
  message?: string;
}

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { hasError: true, message: error.message };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    // eslint-disable-next-line no-console
    console.error("Unhandled error", error, info);
  }

  handleReset = () => {
    this.setState({ hasError: false, message: undefined });
    window.location.assign("/");
  };

  render() {
    if (this.state.hasError) {
      return (
        <div className="p-6 space-y-3">
          <h1 className="text-xl font-semibold text-destructive">Something went wrong</h1>
          <p className="text-sm text-muted-foreground">
            {this.state.message ?? "An unexpected error occurred."}
          </p>
          <button
            type="button"
            onClick={this.handleReset}
            className="px-3 py-2 rounded-md border text-sm"
          >
            Reload
          </button>
        </div>
      );
    }

    return this.props.children;
  }
}
