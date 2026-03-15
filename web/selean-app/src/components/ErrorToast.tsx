import { useCallback, useEffect, useState } from "react";
import { colors, fontSizes, radii, shadows, spacing } from "../theme";

export interface ToastMessage {
  id: number;
  text: string;
}

let nextId = 1;

type Listener = (msg: ToastMessage) => void;
const listeners: Set<Listener> = new Set();

/** Shows an error toast visible to the user. Also logs to console. */
export function showError(message: string): void {
  console.warn(message);
  const msg: ToastMessage = { id: nextId++, text: message };
  for (const fn of listeners) {
    fn(msg);
  }
}

/** Renders a stack of auto-dismissing error toasts. */
export function ErrorToast() {
  const [toasts, setToasts] = useState<ToastMessage[]>([]);

  useEffect(() => {
    const handler: Listener = (msg) => {
      setToasts((prev) => [...prev.slice(-4), msg]);
    };
    listeners.add(handler);
    return () => {
      listeners.delete(handler);
    };
  }, []);

  // Auto-dismiss after 5 seconds.
  useEffect(() => {
    if (toasts.length === 0) return;
    const oldest = toasts[0];
    const timer = setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== oldest.id));
    }, 5000);
    return () => clearTimeout(timer);
  }, [toasts]);

  const dismiss = useCallback((id: number) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  if (toasts.length === 0) return null;

  return (
    <div style={containerStyle}>
      {toasts.map((t) => (
        <div key={t.id} style={toastStyle} onClick={() => dismiss(t.id)}>
          {t.text}
        </div>
      ))}
    </div>
  );
}

const containerStyle: React.CSSProperties = {
  position: "fixed",
  bottom: spacing.lg,
  right: spacing.lg,
  zIndex: 9999,
  display: "flex",
  flexDirection: "column",
  gap: spacing.sm,
  maxWidth: 400,
};

const toastStyle: React.CSSProperties = {
  background: colors.danger,
  color: colors.white,
  fontSize: fontSizes.sm,
  padding: `${spacing.sm}px ${spacing.md}px`,
  borderRadius: radii.md,
  cursor: "pointer",
  boxShadow: shadows.md,
  border: `1px solid ${colors.danger}`,
};
