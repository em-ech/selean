/**
 * Operation buffer for tracking unacknowledged operations.
 *
 * Queues operations that have been optimistically applied locally but
 * not yet confirmed by the server. On `OpAck`, the matching operation
 * is removed from the buffer.
 */

import type { ClientSeqNum } from "./types";

export interface PendingOp {
  clientSeq: ClientSeqNum;
  pageId: string;
  descriptor: Record<string, unknown>;
}

export class OperationBuffer {
  private pending: PendingOp[] = [];
  private nextClientSeq: ClientSeqNum = 1;

  /** Enqueues an operation and returns the assigned client sequence number. */
  enqueue(descriptor: Record<string, unknown>, pageId: string): ClientSeqNum {
    const clientSeq = this.nextClientSeq++;
    this.pending.push({ clientSeq, pageId, descriptor });
    return clientSeq;
  }

  /** Removes the operation with the given client sequence number. */
  acknowledge(clientSeq: ClientSeqNum): void {
    this.pending = this.pending.filter((op) => op.clientSeq !== clientSeq);
  }

  /** Removes all operations with the given client sequence numbers. */
  acknowledgeAll(clientSeqs: ClientSeqNum[]): void {
    const seqSet = new Set(clientSeqs);
    this.pending = this.pending.filter((op) => !seqSet.has(op.clientSeq));
  }

  /** Returns whether there are any unacknowledged operations. */
  hasPending(): boolean {
    return this.pending.length > 0;
  }

  /** Returns the number of unacknowledged operations. */
  pendingCount(): number {
    return this.pending.length;
  }

  /** Returns a copy of all pending operations. */
  getPending(): PendingOp[] {
    return [...this.pending];
  }

  /** Clears all pending operations. */
  clear(): void {
    this.pending = [];
    this.nextClientSeq = 1;
  }
}
