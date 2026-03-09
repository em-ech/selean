import { useCallback, useRef, useState } from "react";
import { colors, fontSizes } from "../theme";
import { authFetch } from "../utils/api";
import type { SeleanEditor } from "../wasm/types";
import {
  useFileImportHandler,
  useFileExportHandler,
  importFileViaUpload,
} from "../hooks/useFileOperations";

interface FileMenuProps {
  editorRef: React.RefObject<SeleanEditor | null>;
  onSceneChanged: () => void;
  onClearAutoSave?: () => void;
}

/**
 * File menu dropdown in the header.
 * Provides New, Save, Open, and import/export for PPTX, IDML, InDesign, and Figma.
 */
export function FileMenu({
  editorRef,
  onSceneChanged,
  onClearAutoSave,
}: FileMenuProps) {
  const [open, setOpen] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const pptxInputRef = useRef<HTMLInputElement>(null);
  const idmlInputRef = useRef<HTMLInputElement>(null);
  const inddInputRef = useRef<HTMLInputElement>(null);

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
    onClearAutoSave?.();
    onSceneChanged();
    close();
  }, [editorRef, onSceneChanged, onClearAutoSave, close]);

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
      } catch (e) {
        console.warn("file-menu:open failed", e);
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

  const handlePptxSelected = useFileImportHandler(
    editorRef,
    "/api/import/pptx",
    onSceneChanged,
    "import-pptx",
  );

  const handleExportPptx = useFileExportHandler(
    editorRef,
    "/api/export/pptx",
    "document.pptx",
    "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    "export-pptx",
    close,
  );

  const handleImportIdml = useCallback(() => {
    idmlInputRef.current?.click();
    close();
  }, [close]);

  const handleIdmlSelected = useFileImportHandler(
    editorRef,
    "/api/import/idml",
    onSceneChanged,
    "import-idml",
  );

  const handleExportIdml = useFileExportHandler(
    editorRef,
    "/api/export/idml",
    "document.idml",
    "application/octet-stream",
    "export-idml",
    close,
  );

  const handleImportIndd = useCallback(() => {
    inddInputRef.current?.click();
    close();
  }, [close]);

  const handleInddSelected = useCallback(
    async (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0];
      if (!file) return;
      const editor = editorRef.current;
      if (!editor) return;
      const ext = file.name.toLowerCase().split(".").pop();
      try {
        if (ext === "idml") {
          await importFileViaUpload(
            editor,
            file,
            "/api/import/idml",
            onSceneChanged,
            "import-idml",
          );
        } else {
          const formData = new FormData();
          formData.append("file", file);
          const response = await authFetch("/api/import/indd", {
            method: "POST",
            body: formData,
          });
          if (response.status === 501) {
            const body = await response.json();
            window.alert(
              body.error ||
                "InDesign Server not configured. Export as IDML instead.",
            );
          } else if (!response.ok) {
            throw new Error(`InDesign import failed: ${response.status}`);
          } else {
            const json = await response.text();
            editor.import_document(json);
            onSceneChanged();
          }
        }
      } catch (err) {
        console.warn("file-menu:import-indesign failed", err);
      }
      e.target.value = "";
    },
    [editorRef, onSceneChanged],
  );

  const handleExportFigma = useFileExportHandler(
    editorRef,
    "/api/export/figma",
    "document.selean-figma.json",
    "application/json",
    "export-figma",
    close,
  );

  const handleImportFigma = useCallback(async () => {
    const input = window.prompt("Enter Figma file URL or key:");
    if (!input) {
      close();
      return;
    }
    const editor = editorRef.current;
    if (!editor) {
      close();
      return;
    }
    // Extract file key from URL or use raw input.
    let fileKey = input.trim();
    const urlMatch = fileKey.match(/\/(?:file|design)\/([a-zA-Z0-9]+)/);
    if (urlMatch) {
      fileKey = urlMatch[1];
    }
    try {
      const response = await authFetch("/api/import/figma", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ file_key: fileKey }),
      });
      if (!response.ok) {
        const status = response.status;
        if (status === 403) {
          window.alert(
            "Figma access denied. Check your access token permissions.",
          );
        } else if (status === 404) {
          window.alert("Figma file not found. Check the file key or URL.");
        } else if (status === 500) {
          window.alert(
            "Server error. Ensure FIGMA_ACCESS_TOKEN is configured.",
          );
        } else {
          window.alert(`Figma import failed: ${status}`);
        }
        close();
        return;
      }
      const json = await response.text();
      editor.import_document(json);
      onSceneChanged();
    } catch (e) {
      console.warn("file-menu:import-figma failed", e);
    }
    close();
  }, [editorRef, onSceneChanged, close]);

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
            <div style={dividerStyle} />
            <button style={menuItemStyle} onClick={handleImportIdml}>
              Import IDML...
            </button>
            <button style={menuItemStyle} onClick={handleExportIdml}>
              Export IDML
            </button>
            <div style={dividerStyle} />
            <button style={menuItemStyle} onClick={handleImportIndd}>
              Import InDesign...
            </button>
            <div style={dividerStyle} />
            <button style={menuItemStyle} onClick={handleImportFigma}>
              Import Figma...
            </button>
            <button style={menuItemStyle} onClick={handleExportFigma}>
              Export to Figma
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
      <input
        ref={idmlInputRef}
        type="file"
        accept=".idml"
        style={{ display: "none" }}
        onChange={handleIdmlSelected}
      />
      <input
        ref={inddInputRef}
        type="file"
        accept=".indd,.idml"
        style={{ display: "none" }}
        onChange={handleInddSelected}
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
