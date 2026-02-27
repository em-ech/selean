import { Component } from "react";
import { colors, fontSizes } from "../theme";

interface ErrorBoundaryProps {
  name: string;
  children: React.ReactNode;
}

interface ErrorBoundaryState {
  error: Error | null;
}

/**
 * Catches rendering errors in child components and displays a fallback
 * instead of crashing the entire application.
 */
export class ErrorBoundary extends Component<
  ErrorBoundaryProps,
  ErrorBoundaryState
> {
  constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = { error: null };
  }

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error };
  }

  override componentDidCatch(error: Error, info: React.ErrorInfo): void {
    console.error(
      `[${this.props.name}] render error:`,
      error,
      info.componentStack,
    );
  }

  override render() {
    if (this.state.error) {
      return (
        <div style={fallbackStyle}>
          <div style={titleStyle}>{this.props.name} crashed</div>
          <div style={messageStyle}>{this.state.error.message}</div>
          <button
            style={retryStyle}
            onClick={() => this.setState({ error: null })}
          >
            Retry
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}

const fallbackStyle: React.CSSProperties = {
  padding: 16,
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  justifyContent: "center",
  gap: 8,
  color: colors.textDim,
  fontSize: fontSizes.sm,
  minHeight: 80,
};

const titleStyle: React.CSSProperties = {
  fontWeight: 600,
  color: colors.text,
};

const messageStyle: React.CSSProperties = {
  fontSize: fontSizes.xs,
  color: colors.textFaint,
  maxWidth: 200,
  textAlign: "center",
  wordBreak: "break-word",
};

const retryStyle: React.CSSProperties = {
  background: colors.border,
  border: `1px solid ${colors.borderHover}`,
  color: colors.text,
  padding: "4px 12px",
  borderRadius: 4,
  cursor: "pointer",
  fontSize: fontSizes.xs,
};
