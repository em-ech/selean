import { useCallback, useEffect, useRef } from "react";
import type { SeleanEditor } from "../wasm/types";

const STORAGE_KEY = "selean_autosave";
const SAVE_INTERVAL_MS = 5000;

interface UseAutoSaveOptions {
  editorRef: React.RefObject<SeleanEditor | null>;
  isReady: boolean;
}

interface UseAutoSaveResult {
  markDirty: () => void;
  loadSavedDocument: () => string | null;
  clearSavedDocument: () => void;
}

/**
 * Auto-saves the document to localStorage on a 5-second interval when dirty.
 * Provides methods to load/clear the saved document.
 */
export function useAutoSave({
  editorRef,
  isReady,
}: UseAutoSaveOptions): UseAutoSaveResult {
  const dirtyRef = useRef(false);

  const markDirty = useCallback(() => {
    dirtyRef.current = true;
  }, []);

  const loadSavedDocument = useCallback((): string | null => {
    try {
      return localStorage.getItem(STORAGE_KEY);
    } catch {
      return null;
    }
  }, []);

  const clearSavedDocument = useCallback(() => {
    try {
      localStorage.removeItem(STORAGE_KEY);
    } catch {
      // storage access failed
    }
  }, []);

  useEffect(() => {
    if (!isReady) return;

    const interval = setInterval(() => {
      if (!dirtyRef.current) return;
      const editor = editorRef.current;
      if (!editor) return;

      try {
        const json = editor.export_document_json();
        localStorage.setItem(STORAGE_KEY, json);
        dirtyRef.current = false;
      } catch {
        // Quota exceeded or storage unavailable. Skip this save.
      }
    }, SAVE_INTERVAL_MS);

    return () => clearInterval(interval);
  }, [editorRef, isReady]);

  return { markDirty, loadSavedDocument, clearSavedDocument };
}
