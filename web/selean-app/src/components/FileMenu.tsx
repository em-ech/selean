import { useCallback, useRef, useState } from "react";
import { colors, fontSizes } from "../theme";
import type { SeleanEditor } from "../wasm/types";

interface FileMenuProps {
  editorRef: React.RefObject<SeleanEditor | null>;
  onSceneChanged: () => void;
}

/**
 * File menu dropdown in the header.
 * Provides New, Save, Open, Import PPTX, Export PPTX.
 */
export function FileMenu({ editorRef, onSceneChanged }: FileMenuProps) {
  const [open, setOpen] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const pptxInputRef = useRef<HTMLInputElement>(null);

  const close = useCallback(() => setOpen(false), []);

  const handleNew = useCallback(() => {
    const editor = editorRef.current;
    if (!editor) return;
    editor.import_document(
      JSON.stringify({
        format_version: 2,
        pages: [
          {
            id: crypto.randomUUID(),
            name: "Page 1",
            width: 1920,
            height: 1080,
            scene: { nodes: {}, roots: [] },
          },
        ],
      }),
    );
    onSceneChanged();
    close();
  }, [editorRef, onSceneChanged, close]);

  const handleSave = useCallback(() => {
    const editor = editorRef.current;
    if (!editor) return;
    const json = editor.export_document_json();
    const blob = new Blob([json], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "document.selean";
    a.click();
    URL.revokeObjectURL(url);
    close();
  }, [editorRef, close]);

  const handleOpen = useCallback(() => {
    fileInputRef.current?.click();
    close();
  }, [close]);

  const handleFileSelected = useCallback(
    async (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0];
      if (!file) return;
      const editor = editorRef.current;
      if (!editor) return;
      try {
        const text = await file.text();
        editor.import_document(text);
        onSceneChanged();
      } catch {
        // File read failed
      }
      // Reset the input so re-selecting the same file works
      e.target.value = "";
    },
    [editorRef, onSceneChanged],
  );

  const handleImportPptx = useCallback(() => {
    pptxInputRef.current?.click();
    close();
  }, [close]);

  const handlePptxSelected = useCallback(
    async (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0];
      if (!file) return;
      const editor = editorRef.current;
      if (!editor) return;
      try {
        const formData = new FormData();
        formData.append("file", file);
        const response = await fetch("/api/import/pptx", {
          method: "POST",
          body: formData,
        });
        if (!response.ok) {
          throw new Error(`Import failed: ${response.status}`);
        }
        const json = await response.text();
        editor.import_document(json);
        onSceneChanged();
      } catch {
        // Import failed
      }
      e.target.value = "";
    },
    [editorRef, onSceneChanged],
  );

  const handleExportPptx = useCallback(async () => {
    const editor = editorRef.current;
    if (!editor) return;
    try {
      const docJson = editor.export_document_json();
      const response = await fetch("/api/export/pptx", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: docJson,
      });
      if (!response.ok) {
        throw new Error(`Export failed: ${response.status}`);
      }
      const blob = await response.blob();
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = "document.pptx";
      a.click();
      URL.revokeObjectURL(url);
    } catch {
      // Export failed
    }
    close();
  }, [editorRef, close]);

  return (
    <div style={containerStyle}>
      <button style={triggerStyle} onClick={() => setOpen(!open)}>
        File
      </button>
      {open && (
        <>
          <div style={backdropStyle} onClick={close} />
          <div style={menuStyle}>
            <button style={menuItemStyle} onClick={handleNew}>
              New
            </button>
            <button style={menuItemStyle} onClick={handleSave}>
              Save
            </button>
            <button style={menuItemStyle} onClick={handleOpen}>
              Open...
            </button>
            <div style={dividerStyle} />
            <button style={menuItemStyle} onClick={handleImportPptx}>
              Import PPTX...
            </button>
            <button style={menuItemStyle} onClick={handleExportPptx}>
              Export PPTX
            </button>
          </div>
        </>
      )}
      <input
        ref={fileInputRef}
        type="file"
        accept=".selean,.json"
        style={{ display: "none" }}
        onChange={handleFileSelected}
      />
      <input
        ref={pptxInputRef}
        type="file"
        accept=".pptx"
        style={{ display: "none" }}
        onChange={handlePptxSelected}
      />
    </div>
  );
}

const containerStyle: React.CSSProperties = {
  position: "relative",
};

const triggerStyle: React.CSSProperties = {
  background: "none",
  border: "none",
  color: colors.text,
  cursor: "pointer",
  fontSize: fontSizes.base,
  padding: "4px 8px",
  borderRadius: 4,
};

const backdropStyle: React.CSSProperties = {
  position: "fixed",
  inset: 0,
  zIndex: 99,
};

const menuStyle: React.CSSProperties = {
  position: "absolute",
  top: "100%",
  left: 0,
  marginTop: 4,
  background: colors.surface,
  border: `1px solid ${colors.border}`,
  borderRadius: 6,
  padding: "4px 0",
  zIndex: 100,
  minWidth: 160,
  boxShadow: "0 4px 12px rgba(0,0,0,0.4)",
};

const menuItemStyle: React.CSSProperties = {
  display: "block",
  width: "100%",
  background: "none",
  border: "none",
  color: colors.text,
  cursor: "pointer",
  fontSize: fontSizes.sm,
  padding: "6px 16px",
  textAlign: "left",
};

const dividerStyle: React.CSSProperties = {
  height: 1,
  background: colors.border,
  margin: "4px 0",
};
