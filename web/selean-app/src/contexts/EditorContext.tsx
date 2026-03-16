import { createContext, useContext } from "react";
import type { SeleanEditor } from "../wasm/types";

interface EditorContextValue {
  editorRef: React.RefObject<SeleanEditor | null>;
  onSceneChanged: () => void;
}

const EditorContext = createContext<EditorContextValue | null>(null);

/** Provides the WASM editor ref and scene-changed callback to descendants. */
export const EditorProvider = EditorContext.Provider;

/**
 * Access the editor ref and onSceneChanged from context.
 * Returns null if no EditorProvider is present (e.g., during loading).
 */
export function useEditor(): EditorContextValue | null {
  return useContext(EditorContext);
}

/**
 * Access the editor ref and onSceneChanged, throwing if not available.
 * Use in components that only render when the editor is ready.
 */
export function useRequiredEditor(): EditorContextValue {
  const ctx = useContext(EditorContext);
  if (!ctx) {
    throw new Error("useRequiredEditor must be used within an EditorProvider");
  }
  return ctx;
}
