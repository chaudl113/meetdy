import React from "react";

interface ErrorBoundaryState {
    hasError: boolean;
    error: Error | null;
}

interface ErrorBoundaryProps {
    children: React.ReactNode;
}

/**
 * Global error boundary to catch unhandled React rendering errors.
 * Prevents the entire app from showing a white screen on crash.
 */
export class ErrorBoundary extends React.Component<
    ErrorBoundaryProps,
    ErrorBoundaryState
> {
    constructor(props: ErrorBoundaryProps) {
        super(props);
        this.state = { hasError: false, error: null };
    }

    static getDerivedStateFromError(error: Error): ErrorBoundaryState {
        return { hasError: true, error };
    }

    componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
        console.error("Unhandled error caught by ErrorBoundary:", error, errorInfo);
    }

    handleReset = () => {
        this.setState({ hasError: false, error: null });
    };

    render() {
        if (this.state.hasError) {
            return (
                <div
                    style={{
                        display: "flex",
                        flexDirection: "column",
                        alignItems: "center",
                        justifyContent: "center",
                        height: "100vh",
                        padding: "2rem",
                        fontFamily:
                            '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif',
                        color: "#e0e0e0",
                        backgroundColor: "#1a1a2e",
                        textAlign: "center",
                    }}
                >
                    <div style={{ fontSize: "48px", marginBottom: "1rem" }}>⚠️</div>
                    <h1
                        style={{
                            fontSize: "1.5rem",
                            fontWeight: 600,
                            marginBottom: "0.5rem",
                        }}
                    >
                        Something went wrong
                    </h1>
                    <p
                        style={{
                            fontSize: "0.9rem",
                            color: "#a0a0b0",
                            marginBottom: "1.5rem",
                            maxWidth: "400px",
                        }}
                    >
                        The application encountered an unexpected error. You can try
                        reloading.
                    </p>
                    {this.state.error && (
                        <pre
                            style={{
                                fontSize: "0.75rem",
                                color: "#ff6b6b",
                                backgroundColor: "#2a2a3e",
                                padding: "0.75rem 1rem",
                                borderRadius: "8px",
                                maxWidth: "500px",
                                overflow: "auto",
                                marginBottom: "1.5rem",
                                textAlign: "left",
                            }}
                        >
                            {this.state.error.message}
                        </pre>
                    )}
                    <button
                        onClick={this.handleReset}
                        style={{
                            padding: "0.6rem 1.5rem",
                            fontSize: "0.9rem",
                            fontWeight: 500,
                            color: "#fff",
                            backgroundColor: "#4a4aff",
                            border: "none",
                            borderRadius: "8px",
                            cursor: "pointer",
                            transition: "background-color 0.2s",
                        }}
                        onMouseOver={(e) =>
                            (e.currentTarget.style.backgroundColor = "#5a5aff")
                        }
                        onMouseOut={(e) =>
                            (e.currentTarget.style.backgroundColor = "#4a4aff")
                        }
                    >
                        Try Again
                    </button>
                </div>
            );
        }

        return this.props.children;
    }
}
