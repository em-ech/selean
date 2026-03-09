import { useEffect, useRef } from "react";
import { authFetch } from "../utils/api";
import type { SeleanEditor } from "../wasm/types";

/**
 * Watches the scene for non-"Inter" font_family values and loads the
 * corresponding font files from the server. Loaded fonts are registered
 * with the WASM editor and cached so each family is fetched only once.
 *
 * @param editorRef - Ref to the WASM editor instance.
 * @param sceneJson - Current scene JSON string (re-parsed on change).
 */
export function useFontLoader(
  editorRef: React.RefObject<SeleanEditor | null>,
  sceneJson: string | null,
): void {
  const loadedFonts = useRef<Set<string>>(new Set(["inter"]));

  useEffect(() => {
    if (!sceneJson || !editorRef.current) return;

    const families = extractFontFamilies(sceneJson);
    const editor = editorRef.current;

    for (const family of families) {
      const key = family.toLowerCase();
      if (loadedFonts.current.has(key)) continue;

      // Mark as loading immediately to prevent duplicate fetches.
      loadedFonts.current.add(key);

      fetchAndRegisterFont(editor, key, family).catch(() => {
        // Remove from cache so a retry is possible on next render.
        loadedFonts.current.delete(key);
      });
    }
  }, [sceneJson, editorRef]);
}

/**
 * Extracts unique font_family values from a scene JSON string.
 * Uses a simple regex to avoid parsing the full JSON tree.
 */
export function extractFontFamilies(json: string): string[] {
  const matches = json.matchAll(/"font_family"\s*:\s*"([^"]+)"/g);
  const families = new Set<string>();
  for (const match of matches) {
    families.add(match[1]);
  }
  return Array.from(families);
}

async function fetchAndRegisterFont(
  editor: SeleanEditor,
  key: string,
  family: string,
): Promise<void> {
  const response = await authFetch(`/api/fonts/${key}`);
  if (!response.ok) {
    console.warn(`Failed to load font "${family}": ${response.status}`);
    return;
  }

  const buffer = await response.arrayBuffer();
  const data = new Uint8Array(buffer);
  const ok = editor.register_font(family, data);
  if (!ok) {
    console.warn(`Failed to register font "${family}" in WASM engine`);
  }
}
