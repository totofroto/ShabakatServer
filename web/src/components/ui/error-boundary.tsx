import { Component, ErrorInfo, ReactNode } from "react";
import { AlertCircle, RotateCcw } from "lucide-react";
import { Button } from "./button";

interface Props {
  children?: ReactNode;
  fallback?: ReactNode;
  onRetry?: () => void;
}

interface State {
  hasError: boolean;
  error?: Error;
}

export class ErrorBoundary extends Component<Props, State> {
  public state: State = {
    hasError: false,
  };

  public static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  public componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error("Uncaught error:", error, errorInfo);
  }

  private handleRetry = () => {
    this.setState({ hasError: false, error: undefined });
    if (this.onRetry) {
      this.onRetry();
    }
  };

  private onRetry = this.props.onRetry;

  public render() {
    if (this.state.hasError) {
      if (this.props.fallback) {
        return this.props.fallback;
      }

      return (
        <div className="flex flex-col items-center justify-center p-6 text-center space-y-4">
          <div className="w-12 h-12 rounded-full bg-error/10 flex items-center justify-center">
            <AlertCircle className="w-6 h-6 text-error" />
          </div>
          <div className="space-y-2">
            <h3 className="text-[15px] font-bold text-primary">Something went wrong</h3>
            <p className="text-[13px] text-secondary">
              {this.state.error?.message || "An unexpected error occurred while loading this component."}
            </p>
          </div>
          <Button
            variant="outline"
            size="sm"
            onClick={this.handleRetry}
            className="flex items-center gap-2"
          >
            <RotateCcw className="w-3.5 h-3.5" />
            Retry
          </Button>
        </div>
      );
    }

    return this.props.children;
  }
}
