/**
 * Type stub for the wasm-pack generated module.
 *
 * This file is replaced by wasm-pack output after running:
 *   npm run wasm:build
 *
 * Kept here so TypeScript compiles before the WASM build.
 */

export default function init(): Promise<void>;

export class SeleanEditor {
  constructor(canvasId: string);
  resize(width: number, height: number): void;
  render(): void;
  on_pointer_move(
    x: number,
    y: number,
    shift: boolean,
    ctrl: boolean,
    alt: boolean,
    meta: boolean,
  ): string;
  on_pointer_down(
    x: number,
    y: number,
    button: number,
    shift: boolean,
    ctrl: boolean,
    alt: boolean,
    meta: boolean,
  ): string;
  on_pointer_up(
    x: number,
    y: number,
    button: number,
    shift: boolean,
    ctrl: boolean,
    alt: boolean,
    meta: boolean,
  ): string;
  on_scroll(
    x: number,
    y: number,
    dx: number,
    dy: number,
    shift: boolean,
    ctrl: boolean,
    alt: boolean,
    meta: boolean,
  ): string;
  execute_command(json: string): boolean;
  undo(): boolean;
  redo(): boolean;
  get_node_json(nodeId: string): string;
  get_selected_ids(): string;
  get_scene_json(): string;
}
