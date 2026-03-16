/** Tool types for creation mode. */
export type ToolType = "select" | "frame" | "text" | "image";

/** Scene node kind constants matching Rust serialization. */
export const NodeKind = {
  Rect: "Rect",
  Frame: "Frame",
  Text: "Text",
  Image: "Image",
  Group: "Group",
  Vector: "Vector",
} as const;
export type NodeKindValue = (typeof NodeKind)[keyof typeof NodeKind];
