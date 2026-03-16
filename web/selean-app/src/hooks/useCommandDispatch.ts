import { useCallback } from "react";
import { useRequiredEditor } from "../contexts/EditorContext";
import { useCollab } from "../collab/CollabContext";

/**
 * Encapsulates WASM command execution + scene refresh + collab broadcast.
 * Components call `dispatch(command)` without knowing about collab details.
 */
export function useCommandDispatch() {
  const { editorRef, onSceneChanged } = useRequiredEditor();
  const collab = useCollab();

  return useCallback(
    (command: Record<string, unknown>) => {
      const editor = editorRef.current;
      if (!editor) return;
      editor.execute_command(JSON.stringify(command));
      onSceneChanged();
      if (collab?.status === "connected") {
        const pageId = editor.active_page_id();
        if (pageId) {
          collab.submitOp(command, pageId);
        }
      }
    },
    [editorRef, onSceneChanged, collab],
  );
}
