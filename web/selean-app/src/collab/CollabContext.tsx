/**
 * React context for sharing collab session state across components.
 *
 * When collab is not connected (or not mounted), consumers get `null`
 * and all mutation paths continue to work in single-user mode.
 */

import { createContext, useContext } from "react";
import type { CollabSession } from "../hooks/useCollabSession";

export const CollabContext = createContext<CollabSession | null>(null);

/** Returns the active collab session, or null if not in collab mode. */
export function useCollab(): CollabSession | null {
  return useContext(CollabContext);
}
