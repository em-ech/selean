import { describe, it, expect } from "vitest";
import { OperationBuffer } from "./op-buffer";

describe("OperationBuffer", () => {
  it("starts empty", () => {
    const buf = new OperationBuffer();
    expect(buf.hasPending()).toBe(false);
    expect(buf.pendingCount()).toBe(0);
    expect(buf.getPending()).toEqual([]);
  });

  it("enqueue assigns sequential client sequence numbers", () => {
    const buf = new OperationBuffer();
    const seq1 = buf.enqueue({ type: "SetFill" }, "page-1");
    const seq2 = buf.enqueue({ type: "SetOpacity" }, "page-1");
    expect(seq1).toBe(1);
    expect(seq2).toBe(2);
    expect(buf.pendingCount()).toBe(2);
    expect(buf.hasPending()).toBe(true);
  });

  it("acknowledge removes the matching operation", () => {
    const buf = new OperationBuffer();
    const seq1 = buf.enqueue({ type: "SetFill" }, "page-1");
    buf.enqueue({ type: "SetOpacity" }, "page-1");
    buf.acknowledge(seq1);
    expect(buf.pendingCount()).toBe(1);
    expect(buf.getPending()[0].clientSeq).toBe(2);
  });

  it("acknowledgeAll removes multiple operations", () => {
    const buf = new OperationBuffer();
    const seq1 = buf.enqueue({ type: "A" }, "page-1");
    const seq2 = buf.enqueue({ type: "B" }, "page-1");
    const seq3 = buf.enqueue({ type: "C" }, "page-1");
    buf.acknowledgeAll([seq1, seq3]);
    expect(buf.pendingCount()).toBe(1);
    expect(buf.getPending()[0].clientSeq).toBe(seq2);
  });

  it("getPending returns a copy, not a reference", () => {
    const buf = new OperationBuffer();
    buf.enqueue({ type: "SetFill" }, "page-1");
    const pending = buf.getPending();
    pending.pop();
    expect(buf.pendingCount()).toBe(1);
  });

  it("clear resets all state", () => {
    const buf = new OperationBuffer();
    buf.enqueue({ type: "SetFill" }, "page-1");
    buf.enqueue({ type: "SetOpacity" }, "page-1");
    buf.clear();
    expect(buf.hasPending()).toBe(false);
    expect(buf.pendingCount()).toBe(0);
    // Sequence numbers restart after clear.
    const seq = buf.enqueue({ type: "Next" }, "page-1");
    expect(seq).toBe(1);
  });

  it("stores pageId and descriptor in pending ops", () => {
    const buf = new OperationBuffer();
    const desc = { type: "SetFill", node_id: "abc", fill: null };
    buf.enqueue(desc, "page-42");
    const pending = buf.getPending();
    expect(pending[0].pageId).toBe("page-42");
    expect(pending[0].descriptor).toEqual(desc);
  });

  it("acknowledge of non-existent seq is a no-op", () => {
    const buf = new OperationBuffer();
    buf.enqueue({ type: "A" }, "p");
    buf.acknowledge(999);
    expect(buf.pendingCount()).toBe(1);
  });
});
