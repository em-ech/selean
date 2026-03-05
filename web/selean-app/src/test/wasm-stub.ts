/**
 * Stub for the WASM package during testing.
 *
 * Vite's import analysis resolves dynamic imports at transform time,
 * so we redirect the WASM package path to this stub via a resolve alias
 * in vitest.config.ts.
 */
export default function init() {
  return Promise.resolve();
}

export const SeleanEditor = {
  create: () => Promise.resolve(null),
};
