import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { WsClient } from "./ws-client";
import type { ServerMessage } from "./types";

// Minimal WebSocket mock for testing.
class MockWebSocket {
  static CONNECTING = 0;
  static OPEN = 1;
  static CLOSING = 2;
  static CLOSED = 3;
  static instances: MockWebSocket[] = [];
  readyState = 0; // CONNECTING
  onopen: (() => void) | null = null;
  onclose: (() => void) | null = null;
  onmessage: ((event: { data: string }) => void) | null = null;
  onerror: (() => void) | null = null;
  sentMessages: string[] = [];

  constructor(public url: string) {
    MockWebSocket.instances.push(this);
  }

  send(data: string) {
    this.sentMessages.push(data);
  }

  close() {
    this.readyState = 3; // CLOSED
    this.onclose?.();
  }

  simulateOpen() {
    this.readyState = 1; // OPEN
    this.onopen?.();
  }

  simulateMessage(msg: ServerMessage) {
    this.onmessage?.({ data: JSON.stringify(msg) });
  }

  simulateClose() {
    this.readyState = 3;
    this.onclose?.();
  }

  simulateError() {
    this.onerror?.();
  }
}

beforeEach(() => {
  MockWebSocket.instances = [];
  vi.stubGlobal("WebSocket", MockWebSocket);
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe("WsClient", () => {
  it("starts disconnected", () => {
    const client = new WsClient();
    expect(client.getStatus()).toBe("disconnected");
  });

  it("transitions to connecting then connected on open", () => {
    const statuses: string[] = [];
    const client = new WsClient({
      onStatusChange: (s) => statuses.push(s),
    });
    client.connect("ws://localhost:8080/api/ws");
    expect(statuses).toContain("connecting");
    MockWebSocket.instances[0].simulateOpen();
    expect(statuses).toContain("connected");
    expect(client.getStatus()).toBe("connected");
    client.disconnect();
  });

  it("send returns false when not connected", () => {
    const client = new WsClient();
    expect(client.send({ type: "LeaveRoom" })).toBe(false);
  });

  it("send returns true when connected", () => {
    const client = new WsClient();
    client.connect("ws://localhost:8080/api/ws");
    MockWebSocket.instances[0].simulateOpen();
    const ok = client.send({ type: "LeaveRoom" });
    expect(ok).toBe(true);
    expect(MockWebSocket.instances[0].sentMessages.length).toBe(1);
    const parsed = JSON.parse(MockWebSocket.instances[0].sentMessages[0]);
    expect(parsed.type).toBe("LeaveRoom");
    client.disconnect();
  });

  it("invokes onMessage for received server messages", () => {
    const messages: ServerMessage[] = [];
    const client = new WsClient({
      onMessage: (msg) => messages.push(msg),
    });
    client.connect("ws://localhost:8080/api/ws");
    MockWebSocket.instances[0].simulateOpen();
    MockWebSocket.instances[0].simulateMessage({
      type: "Error",
      message: "test error",
    });
    expect(messages.length).toBe(1);
    expect(messages[0].type).toBe("Error");
    client.disconnect();
  });

  it("disconnect stops reconnection", () => {
    const client = new WsClient();
    client.connect("ws://localhost:8080/api/ws");
    client.disconnect();
    expect(client.getStatus()).toBe("disconnected");
  });

  it("schedules reconnect on close when shouldReconnect is true", () => {
    const statuses: string[] = [];
    const client = new WsClient({
      onStatusChange: (s) => statuses.push(s),
    });
    client.connect("ws://localhost:8080/api/ws");
    MockWebSocket.instances[0].simulateOpen();
    // Simulate unexpected close.
    MockWebSocket.instances[0].simulateClose();
    expect(statuses.includes("disconnected")).toBe(true);
    // Advance timers to trigger reconnect.
    vi.advanceTimersByTime(1500);
    // A new WebSocket should have been created.
    expect(MockWebSocket.instances.length).toBe(2);
    client.disconnect();
  });

  it("ignores malformed messages", () => {
    const messages: ServerMessage[] = [];
    const client = new WsClient({
      onMessage: (msg) => messages.push(msg),
    });
    client.connect("ws://localhost:8080/api/ws");
    MockWebSocket.instances[0].simulateOpen();
    // Send non-JSON data.
    MockWebSocket.instances[0].onmessage?.({ data: "not json" });
    expect(messages.length).toBe(0);
    client.disconnect();
  });

  it("does not call onReconnect on first connect", () => {
    const onReconnect = vi.fn();
    const client = new WsClient({ onReconnect });
    client.connect("ws://localhost:8080/api/ws");
    MockWebSocket.instances[0].simulateOpen();
    expect(onReconnect).not.toHaveBeenCalled();
    client.disconnect();
  });

  it("calls onReconnect on reconnection", () => {
    const onReconnect = vi.fn();
    const client = new WsClient({ onReconnect });
    client.connect("ws://localhost:8080/api/ws");
    MockWebSocket.instances[0].simulateOpen();
    // Simulate unexpected close.
    MockWebSocket.instances[0].simulateClose();
    // Advance timers to trigger reconnect.
    vi.advanceTimersByTime(1500);
    expect(MockWebSocket.instances.length).toBe(2);
    // Simulate the new connection opening.
    MockWebSocket.instances[1].simulateOpen();
    expect(onReconnect).toHaveBeenCalledOnce();
    client.disconnect();
  });

  it("uses exponential backoff for reconnection delay", () => {
    const client = new WsClient();
    client.connect("ws://localhost:8080/api/ws");
    MockWebSocket.instances[0].simulateOpen();
    // First disconnect: 1s delay.
    MockWebSocket.instances[0].simulateClose();
    vi.advanceTimersByTime(900);
    expect(MockWebSocket.instances.length).toBe(1);
    vi.advanceTimersByTime(200);
    expect(MockWebSocket.instances.length).toBe(2);
    // Second disconnect: 2s delay.
    MockWebSocket.instances[1].simulateClose();
    vi.advanceTimersByTime(1900);
    expect(MockWebSocket.instances.length).toBe(2);
    vi.advanceTimersByTime(200);
    expect(MockWebSocket.instances.length).toBe(3);
    client.disconnect();
  });

  it("caps reconnection delay at maxReconnectDelay", () => {
    const client = new WsClient({ maxReconnectDelay: 2000 });
    client.connect("ws://localhost:8080/api/ws");
    MockWebSocket.instances[0].simulateOpen();
    // Disconnect many times to exceed max.
    for (let i = 0; i < 5; i++) {
      MockWebSocket.instances[i].simulateClose();
      vi.advanceTimersByTime(2100);
      MockWebSocket.instances[i + 1].simulateOpen();
    }
    // After 5 reconnects, delay should still be capped.
    MockWebSocket.instances[5].simulateClose();
    vi.advanceTimersByTime(2100);
    expect(MockWebSocket.instances.length).toBe(7);
    client.disconnect();
  });
});
