import { useCallback, useRef, useState } from "react";
import { colors, fontSizes, radii, spacing } from "../theme";
import { showError } from "./ErrorToast";
import type { SeleanEditor } from "../wasm/types";

type TabId = "elements" | "templates" | "uploads";

interface LeftSidebarProps {
  editorRef: React.RefObject<SeleanEditor | null>;
  onSceneChanged: () => void;
  isOpen: boolean;
  onToggle: () => void;
  activeWorkspaceId?: string;
  /** Dynamic width from resize handle. */
  width?: number;
}

const SHAPE_ELEMENTS = [
  {
    label: "Rectangle",
    kind: "Rect",
    icon: (
      <svg
        width="20"
        height="20"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
      >
        <rect x="3" y="3" width="18" height="18" rx="2" />
      </svg>
    ),
    defaults: { width: 200, height: 150 },
  },
  {
    label: "Square",
    kind: "Rect",
    icon: (
      <svg
        width="20"
        height="20"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
      >
        <rect x="4" y="4" width="16" height="16" />
      </svg>
    ),
    defaults: { width: 150, height: 150 },
  },
  {
    label: "Frame",
    kind: "Rect",
    icon: (
      <svg
        width="20"
        height="20"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
      >
        <rect x="2" y="2" width="20" height="20" rx="0" />
        <line x1="2" y1="8" x2="22" y2="8" />
      </svg>
    ),
    defaults: { width: 400, height: 300 },
  },
];

const TEXT_ELEMENTS = [
  {
    label: "Heading",
    icon: (
      <svg
        width="20"
        height="20"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
      >
        <path d="M4 4h16M4 12h16M4 20h10" />
      </svg>
    ),
    defaults: { content: "Heading", font_size: 32, width: 300, height: 50 },
  },
  {
    label: "Subheading",
    icon: (
      <svg
        width="20"
        height="20"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
      >
        <path d="M4 6h16M4 14h12" />
      </svg>
    ),
    defaults: { content: "Subheading", font_size: 20, width: 250, height: 35 },
  },
  {
    label: "Body text",
    icon: (
      <svg
        width="20"
        height="20"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
      >
        <path d="M4 6h16M4 10h16M4 14h16M4 18h10" />
      </svg>
    ),
    defaults: {
      content: "Type your text here",
      font_size: 14,
      width: 250,
      height: 24,
    },
  },
];

interface TemplateCard {
  id: string;
  label: string;
  description: string;
  icon: string;
  width: number;
  height: number;
  /** Elements to create on the new page. */
  elements: { kind: string; name: string; [key: string]: unknown }[];
}

const TEMPLATES: TemplateCard[] = [
  {
    id: "social-post",
    label: "Social Media Post",
    description: "1080x1080 square",
    icon: "S",
    width: 1080,
    height: 1080,
    elements: [
      {
        kind: "Rect",
        name: "Background",
        x: 0,
        y: 0,
        width: 1080,
        height: 1080,
        fill: { r: 0.96, g: 0.96, b: 0.98, a: 1 },
      },
      {
        kind: "Text",
        name: "Title",
        x: 140,
        y: 400,
        width: 800,
        height: 80,
        content: "Your Title Here",
        font_size: 48,
      },
      {
        kind: "Text",
        name: "Subtitle",
        x: 240,
        y: 500,
        width: 600,
        height: 40,
        content: "Add your subtitle",
        font_size: 24,
      },
    ],
  },
  {
    id: "presentation",
    label: "Presentation Slide",
    description: "1920x1080 widescreen",
    icon: "P",
    width: 1920,
    height: 1080,
    elements: [
      {
        kind: "Rect",
        name: "Background",
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
        fill: { r: 1, g: 1, b: 1, a: 1 },
      },
      {
        kind: "Text",
        name: "Title",
        x: 120,
        y: 200,
        width: 1680,
        height: 100,
        content: "Presentation Title",
        font_size: 56,
      },
      {
        kind: "Text",
        name: "Body",
        x: 120,
        y: 400,
        width: 1680,
        height: 400,
        content: "Add your content here",
        font_size: 24,
      },
    ],
  },
  {
    id: "business-card",
    label: "Business Card",
    description: "3.5x2 inches",
    icon: "B",
    width: 1050,
    height: 600,
    elements: [
      {
        kind: "Rect",
        name: "Background",
        x: 0,
        y: 0,
        width: 1050,
        height: 600,
        fill: { r: 1, g: 1, b: 1, a: 1 },
      },
      {
        kind: "Text",
        name: "Name",
        x: 60,
        y: 200,
        width: 930,
        height: 50,
        content: "Your Name",
        font_size: 32,
      },
      {
        kind: "Text",
        name: "Title",
        x: 60,
        y: 260,
        width: 930,
        height: 30,
        content: "Job Title",
        font_size: 18,
      },
      {
        kind: "Text",
        name: "Contact",
        x: 60,
        y: 400,
        width: 930,
        height: 60,
        content: "email@example.com\n+1 (555) 000-0000",
        font_size: 14,
      },
    ],
  },
  {
    id: "poster",
    label: "Poster",
    description: "24x36 portrait",
    icon: "A",
    width: 1440,
    height: 2160,
    elements: [
      {
        kind: "Rect",
        name: "Background",
        x: 0,
        y: 0,
        width: 1440,
        height: 2160,
        fill: { r: 0.95, g: 0.93, b: 0.9, a: 1 },
      },
      {
        kind: "Text",
        name: "Headline",
        x: 120,
        y: 300,
        width: 1200,
        height: 120,
        content: "EVENT NAME",
        font_size: 72,
      },
      {
        kind: "Text",
        name: "Details",
        x: 120,
        y: 1600,
        width: 1200,
        height: 200,
        content: "Date / Time / Location",
        font_size: 28,
      },
    ],
  },
  {
    id: "flyer",
    label: "Flyer",
    description: "8.5x11 letter",
    icon: "F",
    width: 1275,
    height: 1650,
    elements: [
      {
        kind: "Rect",
        name: "Background",
        x: 0,
        y: 0,
        width: 1275,
        height: 1650,
        fill: { r: 1, g: 1, b: 1, a: 1 },
      },
      {
        kind: "Text",
        name: "Heading",
        x: 100,
        y: 120,
        width: 1075,
        height: 80,
        content: "Flyer Title",
        font_size: 48,
      },
      {
        kind: "Text",
        name: "Body",
        x: 100,
        y: 300,
        width: 1075,
        height: 800,
        content:
          "Add your content here. Describe your event, product, or announcement.",
        font_size: 18,
      },
    ],
  },
  {
    id: "resume",
    label: "Resume",
    description: "8.5x11 letter",
    icon: "R",
    width: 1275,
    height: 1650,
    elements: [
      {
        kind: "Rect",
        name: "Background",
        x: 0,
        y: 0,
        width: 1275,
        height: 1650,
        fill: { r: 1, g: 1, b: 1, a: 1 },
      },
      {
        kind: "Text",
        name: "Name",
        x: 100,
        y: 80,
        width: 1075,
        height: 60,
        content: "Your Name",
        font_size: 36,
      },
      {
        kind: "Text",
        name: "Contact",
        x: 100,
        y: 150,
        width: 1075,
        height: 30,
        content: "email@example.com | (555) 000-0000 | City, State",
        font_size: 14,
      },
      {
        kind: "Rect",
        name: "Divider",
        x: 100,
        y: 200,
        width: 1075,
        height: 2,
        fill: { r: 0.8, g: 0.8, b: 0.8, a: 1 },
      },
      {
        kind: "Text",
        name: "Experience",
        x: 100,
        y: 240,
        width: 1075,
        height: 30,
        content: "EXPERIENCE",
        font_size: 16,
      },
    ],
  },
];

/** Proportional thumbnail preview for template cards. */
function TemplateThumbnail({
  width,
  height,
}: {
  width: number;
  height: number;
}) {
  const maxDim = 48;
  const aspect = width / height;
  const w = aspect >= 1 ? maxDim : Math.round(maxDim * aspect);
  const h = aspect >= 1 ? Math.round(maxDim / aspect) : maxDim;
  return (
    <div
      style={{
        width: w,
        height: h,
        background: colors.surface,
        border: `1px solid ${colors.border}`,
        borderRadius: radii.sm,
        marginBottom: spacing.xs,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      <svg width={w * 0.6} height={h * 0.4} viewBox="0 0 20 10" fill="none">
        <rect
          x="1"
          y="1"
          width="18"
          height="3"
          rx="0.5"
          fill={colors.borderHover}
        />
        <rect x="1" y="6" width="12" height="2" rx="0.5" fill={colors.border} />
      </svg>
    </div>
  );
}

/**
 * Persistent left sidebar with Elements / Templates / Uploads tabs.
 * Elements: click-to-place shapes and text.
 * Templates: starter layout cards.
 * Uploads: image upload gallery.
 */
export function LeftSidebar({
  editorRef,
  onSceneChanged,
  isOpen,
  onToggle,
  width,
}: LeftSidebarProps) {
  const [activeTab, setActiveTab] = useState<TabId>("elements");
  const imageInputRef = useRef<HTMLInputElement>(null);

  const placeElement = useCallback(
    (kind: string, defaults: Record<string, unknown>) => {
      const editor = editorRef.current;
      if (!editor) return;

      // Place at canvas center using camera.
      // Camera pan_x/pan_y is the world-space center of the viewport.
      let x = 100;
      let y = 100;
      try {
        const cam = editor.get_camera();
        if (cam) {
          const w = (defaults.width as number) ?? 200;
          const h = (defaults.height as number) ?? 150;
          x = cam.pan_x - w / 2;
          y = cam.pan_y - h / 2;
        }
      } catch {
        // Fallback to default position.
      }

      editor.execute_tool_call(
        "create_node",
        JSON.stringify({
          kind,
          name: defaults.content ? String(defaults.content) : kind,
          x,
          y,
          ...defaults,
        }),
      );
      onSceneChanged();
    },
    [editorRef, onSceneChanged],
  );

  const applyTemplate = useCallback(
    (template: TemplateCard) => {
      const editor = editorRef.current;
      if (!editor) return;

      // Create a new page with template dimensions.
      const pageId = editor.add_page(
        template.label,
        template.width,
        template.height,
      );
      if (!pageId) {
        showError("Failed to create template page");
        return;
      }
      editor.set_active_page(pageId);

      // Create all template elements.
      for (const el of template.elements) {
        editor.execute_tool_call("create_node", JSON.stringify(el));
      }

      onSceneChanged();
    },
    [editorRef, onSceneChanged],
  );

  const handleImageUpload = useCallback(
    async (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0];
      if (!file) return;
      const editor = editorRef.current;
      if (!editor) return;

      try {
        const buffer = await file.arrayBuffer();
        const data = new Uint8Array(buffer);
        const assetRef = `img_${Date.now()}`;
        const ok = editor.register_image_asset(assetRef, data);
        if (ok) {
          placeElement("Image", {
            name: file.name.replace(/\.[^.]+$/, ""),
            width: 400,
            height: 300,
            asset_ref: assetRef,
          });
        }
      } catch (err) {
        showError("Failed to upload image");
        console.warn("image upload failed", err);
      }
      e.target.value = "";
    },
    [editorRef, placeElement],
  );

  if (!isOpen) {
    return (
      <div style={collapsedStyle}>
        <button
          data-interactive
          onClick={onToggle}
          style={collapsedBtnStyle}
          title="Open sidebar"
        >
          <svg
            width="18"
            height="18"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
          >
            <rect x="3" y="3" width="7" height="7" />
            <rect x="14" y="3" width="7" height="7" />
            <rect x="3" y="14" width="7" height="7" />
            <rect x="14" y="14" width="7" height="7" />
          </svg>
        </button>
      </div>
    );
  }

  return (
    <div style={{ ...panelStyle, ...(width ? { width } : {}) }}>
      <div style={headerStyle}>
        <div style={tabsStyle}>
          {(
            [
              ["elements", "Elements"],
              ["templates", "Templates"],
              ["uploads", "Uploads"],
            ] as const
          ).map(([id, label]) => (
            <button
              key={id}
              data-interactive
              onClick={() => setActiveTab(id)}
              style={{
                ...tabStyle,
                ...(activeTab === id ? activeTabStyle : {}),
              }}
            >
              {label}
            </button>
          ))}
        </div>
        <button data-interactive onClick={onToggle} style={closeBtnStyle} title="Close sidebar">
          <svg
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
          >
            <path d="M15 18l-6-6 6-6" />
          </svg>
        </button>
      </div>

      <div style={contentStyle}>
        {activeTab === "elements" && (
          <>
            <div style={sectionLabelStyle}>Shapes</div>
            <div style={gridStyle}>
              {SHAPE_ELEMENTS.map((el) => (
                <button
                  key={el.label}
                  data-interactive
                  onClick={() => placeElement(el.kind, el.defaults)}
                  style={elementBtnStyle}
                  title={el.label}
                >
                  {el.icon}
                  <span style={elementLabelStyle}>{el.label}</span>
                </button>
              ))}
            </div>

            <div style={sectionLabelStyle}>Text</div>
            <div style={gridStyle}>
              {TEXT_ELEMENTS.map((el) => (
                <button
                  key={el.label}
                  data-interactive
                  onClick={() => placeElement("Text", el.defaults)}
                  style={elementBtnStyle}
                  title={el.label}
                >
                  {el.icon}
                  <span style={elementLabelStyle}>{el.label}</span>
                </button>
              ))}
            </div>

            <div style={sectionLabelStyle}>Media</div>
            <div style={gridStyle}>
              <button
                data-interactive
                onClick={() => imageInputRef.current?.click()}
                style={elementBtnStyle}
                title="Upload image"
              >
                <svg
                  width="20"
                  height="20"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                >
                  <rect x="3" y="3" width="18" height="18" rx="2" />
                  <circle cx="8.5" cy="8.5" r="1.5" />
                  <polyline points="21 15 16 10 5 21" />
                </svg>
                <span style={elementLabelStyle}>Image</span>
              </button>
            </div>
            <input
              ref={imageInputRef}
              type="file"
              accept="image/png,image/jpeg,image/webp"
              style={{ display: "none" }}
              onChange={handleImageUpload}
            />
          </>
        )}

        {activeTab === "templates" && (
          <div style={templateGridStyle}>
            {TEMPLATES.map((tmpl) => (
              <button
                key={tmpl.id}
                data-interactive
                style={templateCardStyle}
                title={`${tmpl.label} (${tmpl.width}x${tmpl.height})`}
                onClick={() => applyTemplate(tmpl)}
              >
                <TemplateThumbnail width={tmpl.width} height={tmpl.height} />
                <div style={templateLabelStyle}>{tmpl.label}</div>
                <div style={templateDescStyle}>{tmpl.description}</div>
              </button>
            ))}
          </div>
        )}

        {activeTab === "uploads" && (
          <div style={uploadsStyle}>
            <button
              onClick={() => imageInputRef.current?.click()}
              style={uploadBtnStyle}
            >
              <svg
                width="24"
                height="24"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
              >
                <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
                <polyline points="17 8 12 3 7 8" />
                <line x1="12" y1="3" x2="12" y2="15" />
              </svg>
              Upload image
            </button>
            <input
              ref={imageInputRef}
              type="file"
              accept="image/png,image/jpeg,image/webp"
              style={{ display: "none" }}
              onChange={handleImageUpload}
            />
          </div>
        )}
      </div>
    </div>
  );
}

// --- Styles ---

const panelStyle: React.CSSProperties = {
  width: 260,
  minWidth: 200,
  maxWidth: 360,
  display: "flex",
  flexDirection: "column",
  background: colors.bg,
  borderRight: `1px solid ${colors.border}`,
  overflow: "hidden",
  flexShrink: 0,
};

const collapsedStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  borderRight: `1px solid ${colors.border}`,
  flexShrink: 0,
};

const collapsedBtnStyle: React.CSSProperties = {
  width: 40,
  height: 48,
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  background: "transparent",
  border: "none",
  cursor: "pointer",
  color: colors.textDim,
};

const headerStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  padding: `0 ${spacing.sm}px 0 0`,
  borderBottom: `1px solid ${colors.border}`,
  flexShrink: 0,
  height: 48,
};

const tabsStyle: React.CSSProperties = {
  display: "flex",
  gap: 0,
  flex: 1,
};

const tabStyle: React.CSSProperties = {
  background: "transparent",
  border: "none",
  borderBottom: "2px solid transparent",
  padding: `${spacing.md}px ${spacing.md}px`,
  fontSize: fontSizes.sm,
  color: colors.textDim,
  cursor: "pointer",
  fontWeight: 500,
  whiteSpace: "nowrap",
};

const activeTabStyle: React.CSSProperties = {
  color: colors.accent,
  borderBottom: `2px solid ${colors.accent}`,
  fontWeight: 600,
};

const closeBtnStyle: React.CSSProperties = {
  background: "transparent",
  border: "none",
  cursor: "pointer",
  color: colors.textDim,
  padding: spacing.xs,
  borderRadius: radii.sm,
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
};

const contentStyle: React.CSSProperties = {
  flex: 1,
  overflow: "auto",
  padding: spacing.md,
};

const sectionLabelStyle: React.CSSProperties = {
  fontSize: fontSizes.xs,
  fontWeight: 600,
  color: colors.textDim,
  textTransform: "uppercase",
  letterSpacing: "0.05em",
  marginBottom: spacing.sm,
  marginTop: spacing.md,
};

const gridStyle: React.CSSProperties = {
  display: "grid",
  gridTemplateColumns: "repeat(3, 1fr)",
  gap: spacing.sm,
  marginBottom: spacing.md,
};

const elementBtnStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  gap: spacing.xs,
  padding: spacing.sm,
  background: colors.surface,
  border: `1px solid ${colors.border}`,
  borderRadius: radii.md,
  cursor: "pointer",
  color: colors.text,
  fontSize: fontSizes.xs,
};

const elementLabelStyle: React.CSSProperties = {
  fontSize: fontSizes.xs,
  color: colors.textMuted,
};

const templateGridStyle: React.CSSProperties = {
  display: "grid",
  gridTemplateColumns: "repeat(2, 1fr)",
  gap: spacing.sm,
};

const templateCardStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  padding: spacing.lg,
  background: colors.surface,
  border: `1px solid ${colors.border}`,
  borderRadius: radii.md,
  cursor: "pointer",
  color: colors.text,
  gap: spacing.xs,
};

const templateLabelStyle: React.CSSProperties = {
  fontSize: fontSizes.sm,
  fontWeight: 600,
};

const templateDescStyle: React.CSSProperties = {
  fontSize: fontSizes.xs,
  color: colors.textDim,
};

const uploadsStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  justifyContent: "center",
  padding: spacing.xxl,
  gap: spacing.md,
};

const uploadBtnStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  gap: spacing.sm,
  padding: `${spacing.xl}px ${spacing.xxl}px`,
  background: colors.surface,
  border: `2px dashed ${colors.border}`,
  borderRadius: radii.lg,
  cursor: "pointer",
  color: colors.textMuted,
  fontSize: fontSizes.md,
  fontWeight: 500,
  width: "100%",
};
