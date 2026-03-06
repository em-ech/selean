import { useCallback } from "react";
import type { SeleanEditor } from "../wasm/types";

/**
 * Imports a file by uploading it as multipart FormData to the given endpoint.
 * On success, passes the JSON response to `editor.import_document()`.
 */
export async function importFileViaUpload(
  editor: SeleanEditor,
  file: File,
  endpoint: string,
  onSuccess: () => void,
  label: string,
): Promise<void> {
  const formData = new FormData();
  formData.append("file", file);
  const response = await fetch(endpoint, {
    method: "POST",
    body: formData,
  });
  if (!response.ok) {
    throw new Error(`${label} failed: ${response.status}`);
  }
  const json = await response.text();
  editor.import_document(json);
  onSuccess();
}

/**
 * Exports the current document by POSTing its JSON to the given endpoint
 * and triggering a file download with the response blob.
 */
export async function exportViaFetch(
  editor: SeleanEditor,
  endpoint: string,
  filename: string,
  contentType: string,
  label: string,
): Promise<void> {
  const docJson = editor.export_document_json();
  const response = await fetch(endpoint, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: docJson,
  });
  if (!response.ok) {
    throw new Error(`${label} failed: ${response.status}`);
  }

  const isJson = contentType === "application/json";
  const blob = isJson
    ? new Blob([await response.text()], { type: contentType })
    : await response.blob();

  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}

/**
 * Creates a file-selected callback for multipart import endpoints.
 */
export function useFileImportHandler(
  editorRef: React.RefObject<SeleanEditor | null>,
  endpoint: string,
  onSceneChanged: () => void,
  label: string,
) {
  return useCallback(
    async (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0];
      if (!file) return;
      const editor = editorRef.current;
      if (!editor) return;
      try {
        await importFileViaUpload(
          editor,
          file,
          endpoint,
          onSceneChanged,
          label,
        );
      } catch (err) {
        console.warn(`file-menu:${label} failed`, err);
      }
      e.target.value = "";
    },
    [editorRef, endpoint, onSceneChanged, label],
  );
}

/**
 * Creates an export callback for JSON-body export endpoints.
 */
export function useFileExportHandler(
  editorRef: React.RefObject<SeleanEditor | null>,
  endpoint: string,
  filename: string,
  contentType: string,
  label: string,
  close: () => void,
) {
  return useCallback(async () => {
    const editor = editorRef.current;
    if (!editor) return;
    try {
      await exportViaFetch(editor, endpoint, filename, contentType, label);
    } catch (err) {
      console.warn(`file-menu:${label} failed`, err);
    }
    close();
  }, [editorRef, endpoint, filename, contentType, label, close]);
}
