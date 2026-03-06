// Selean Figma Plugin: Import from Selean
//
// Reads a .selean-figma.json interchange file and creates
// corresponding Figma nodes. The interchange format mirrors
// Figma's node structure for direct mapping.

figma.showUI(__html__, { width: 320, height: 240 });

interface InterchangeColor {
  r: number;
  g: number;
  b: number;
  a: number;
}

interface InterchangeGradientStop {
  position: number;
  color: InterchangeColor;
}

interface InterchangeVector {
  x: number;
  y: number;
}

interface InterchangePaint {
  type: string;
  color?: InterchangeColor;
  opacity?: number;
  gradientHandlePositions?: InterchangeVector[];
  gradientStops?: InterchangeGradientStop[];
}

interface InterchangeEffect {
  type: string;
  radius?: number;
  color?: InterchangeColor;
  offsetX?: number;
  offsetY?: number;
}

interface InterchangeNode {
  type: string;
  name: string;
  x: number;
  y: number;
  width: number;
  height: number;
  visible: boolean;
  opacity: number;
  cornerRadius?: number[];
  fills?: InterchangePaint[];
  strokes?: InterchangePaint[];
  strokeWeight?: number;
  effects?: InterchangeEffect[];
  characters?: string;
  fontSize?: number;
  fontFamily?: string;
  fontWeight?: number;
  fontStyle?: string;
  textAlignHorizontal?: string;
  lineHeightPx?: number;
  textColor?: InterchangeColor;
  pathData?: string;
  assetRef?: string;
  children?: InterchangeNode[];
}

interface InterchangePage {
  name: string;
  width: number;
  height: number;
  children: InterchangeNode[];
}

interface InterchangeFile {
  name: string;
  pages: InterchangePage[];
}

function toRgb(c: InterchangeColor): RGB {
  return { r: c.r, g: c.g, b: c.b };
}

function convertPaintToFigma(paint: InterchangePaint): Paint | null {
  if (paint.type === "SOLID" && paint.color) {
    return {
      type: "SOLID",
      color: toRgb(paint.color),
      opacity: paint.color.a * (paint.opacity ?? 1),
    };
  }

  if (
    (paint.type === "GRADIENT_LINEAR" || paint.type === "GRADIENT_RADIAL") &&
    paint.gradientStops &&
    paint.gradientStops.length > 0
  ) {
    const stops: ColorStop[] = paint.gradientStops.map((s) => ({
      position: s.position,
      color: {
        r: s.color.r,
        g: s.color.g,
        b: s.color.b,
        a: s.color.a * (paint.opacity ?? 1),
      },
    }));

    const positions = paint.gradientHandlePositions ?? [];
    const transform: Transform = [
      [1, 0, positions[0]?.x ?? 0],
      [0, 1, positions[0]?.y ?? 0],
    ];

    return {
      type:
        paint.type === "GRADIENT_LINEAR"
          ? "GRADIENT_LINEAR"
          : "GRADIENT_RADIAL",
      gradientTransform: transform,
      gradientStops: stops,
    } as GradientPaint;
  }

  return null;
}

function convertEffectsToFigma(effects: InterchangeEffect[]): Effect[] {
  return effects
    .map((e): Effect | null => {
      if (e.type === "DROP_SHADOW" && e.color) {
        return {
          type: "DROP_SHADOW",
          visible: true,
          radius: e.radius ?? 0,
          color: {
            r: e.color.r,
            g: e.color.g,
            b: e.color.b,
            a: e.color.a,
          },
          offset: { x: e.offsetX ?? 0, y: e.offsetY ?? 0 },
          blendMode: "NORMAL",
        } as DropShadowEffect;
      }
      if (e.type === "LAYER_BLUR") {
        return {
          type: "LAYER_BLUR",
          visible: true,
          radius: e.radius ?? 0,
        } as BlurEffect;
      }
      return null;
    })
    .filter((e): e is Effect => e !== null);
}

function convertFillsToFigma(paints: InterchangePaint[]): Paint[] {
  return paints.map(convertPaintToFigma).filter((p): p is Paint => p !== null);
}

async function createNode(
  node: InterchangeNode,
  parent: BaseNode & ChildrenMixin,
): Promise<SceneNode | null> {
  let figmaNode: SceneNode;

  switch (node.type) {
    case "FRAME":
    case "RECTANGLE": {
      const frame = figma.createFrame();
      frame.resize(Math.max(1, node.width), Math.max(1, node.height));
      if (node.cornerRadius) {
        if (node.cornerRadius.length === 4) {
          frame.topLeftRadius = node.cornerRadius[0];
          frame.topRightRadius = node.cornerRadius[1];
          frame.bottomRightRadius = node.cornerRadius[2];
          frame.bottomLeftRadius = node.cornerRadius[3];
        }
      }
      figmaNode = frame;
      break;
    }
    case "TEXT": {
      const text = figma.createText();
      const family = node.fontFamily ?? "Inter";
      const weight = node.fontWeight ?? 400;
      const style = node.fontStyle === "italic" ? "Italic" : "Regular";
      const fontName =
        weight === 400
          ? { family, style }
          : {
              family,
              style: `${weightToStyle(weight)}${style === "Italic" ? " Italic" : ""}`,
            };

      try {
        await figma.loadFontAsync(fontName);
        text.fontName = fontName;
      } catch {
        await figma.loadFontAsync({ family: "Inter", style: "Regular" });
        text.fontName = { family: "Inter", style: "Regular" };
      }

      text.characters = node.characters ?? "";
      text.fontSize = node.fontSize ?? 16;

      if (node.lineHeightPx) {
        text.lineHeight = { value: node.lineHeightPx, unit: "PIXELS" };
      }

      if (node.textAlignHorizontal) {
        const align = node.textAlignHorizontal.toUpperCase();
        if (
          align === "CENTER" ||
          align === "LEFT" ||
          align === "RIGHT" ||
          align === "JUSTIFIED"
        ) {
          text.textAlignHorizontal = align;
        }
      }

      if (node.textColor) {
        text.fills = [
          {
            type: "SOLID",
            color: toRgb(node.textColor),
            opacity: node.textColor.a,
          },
        ];
      }

      text.resize(Math.max(1, node.width), Math.max(1, node.height));
      figmaNode = text;
      break;
    }
    case "GROUP": {
      // Create a temporary frame, add children, then group them
      const tempFrame = figma.createFrame();
      tempFrame.resize(Math.max(1, node.width), Math.max(1, node.height));
      parent.appendChild(tempFrame);

      if (node.children && node.children.length > 0) {
        const childNodes: SceneNode[] = [];
        for (const child of node.children) {
          const cn = await createNode(child, tempFrame);
          if (cn) childNodes.push(cn);
        }
        if (childNodes.length > 0) {
          const group = figma.group(childNodes, parent);
          group.name = node.name;
          group.x = node.x;
          group.y = node.y;
          group.opacity = node.opacity;
          group.visible = node.visible;
          tempFrame.remove();
          return group;
        }
      }
      tempFrame.remove();
      return null;
    }
    case "VECTOR": {
      // Create a rectangle as a placeholder for vector paths
      const rect = figma.createRectangle();
      rect.resize(Math.max(1, node.width), Math.max(1, node.height));
      figmaNode = rect;
      break;
    }
    case "IMAGE": {
      const rect = figma.createRectangle();
      rect.resize(Math.max(1, node.width), Math.max(1, node.height));
      figmaNode = rect;
      break;
    }
    default:
      return null;
  }

  figmaNode.name = node.name;
  figmaNode.x = node.x;
  figmaNode.y = node.y;
  figmaNode.opacity = node.opacity;
  figmaNode.visible = node.visible;

  if (node.fills && "fills" in figmaNode) {
    const fills = convertFillsToFigma(node.fills);
    if (fills.length > 0) {
      (figmaNode as GeometryMixin).fills = fills;
    }
  }

  if (node.strokes && "strokes" in figmaNode) {
    const strokes = convertFillsToFigma(node.strokes);
    if (strokes.length > 0) {
      (figmaNode as GeometryMixin).strokes = strokes;
    }
  }

  if (node.strokeWeight != null && "strokeWeight" in figmaNode) {
    (figmaNode as GeometryMixin).strokeWeight = node.strokeWeight;
  }

  if (node.effects && node.effects.length > 0) {
    figmaNode.effects = convertEffectsToFigma(node.effects);
  }

  parent.appendChild(figmaNode);

  // Recursively add children for frames
  if (node.children && node.type === "FRAME" && "children" in figmaNode) {
    for (const child of node.children) {
      await createNode(child, figmaNode as FrameNode);
    }
  }

  return figmaNode;
}

function weightToStyle(weight: number): string {
  if (weight <= 100) return "Thin";
  if (weight <= 200) return "ExtraLight";
  if (weight <= 300) return "Light";
  if (weight <= 400) return "Regular";
  if (weight <= 500) return "Medium";
  if (weight <= 600) return "SemiBold";
  if (weight <= 700) return "Bold";
  if (weight <= 800) return "ExtraBold";
  return "Black";
}

figma.ui.onmessage = async (msg) => {
  if (msg.type !== "import") return;

  const data = msg.data as InterchangeFile;
  if (!data.pages || data.pages.length === 0) {
    figma.ui.postMessage({ type: "error", message: "No pages found in file" });
    return;
  }

  let totalNodes = 0;

  try {
    for (const page of data.pages) {
      const figmaPage = figma.createPage();
      figmaPage.name = page.name;

      for (const child of page.children) {
        const node = await createNode(child, figmaPage);
        if (node) totalNodes++;
      }
    }

    figma.ui.postMessage({
      type: "done",
      count: totalNodes,
      pages: data.pages.length,
    });
  } catch (err) {
    figma.ui.postMessage({
      type: "error",
      message: `Import failed: ${err instanceof Error ? err.message : String(err)}`,
    });
  }
};
