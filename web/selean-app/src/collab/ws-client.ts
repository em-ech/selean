/**
 * WebSocket client for the collaboration protocol.
 *
 * Handles connection, reconnection with exponential backoff, and
 * JSON message serialization/deserialization.
 */

import type { ClientMessage, ServerMessage } from "./types";

export type ConnectionStatus = "disconnected" | "connecting" | "connected";

export interface WsClientOptions {
  /** Called when the connection status changes. */
  onStatusChange?: (status: ConnectionStatus) => void;
  /** Called when a server message is received. */
  onMessage?: (msg: ServerMessage) => void;
  /** Called when a reconnection opens, before any messages are sent. */
  onReconnect?: () => void;
  /** Maximum reconnection delay in ms. Default: 30000. */
  maxReconnectDelay?: number;
}

export class WsClient {
  private ws: WebSocket | null = null;
  private status: ConnectionStatus = "disconnected";
  private url: string = "";
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private reconnectAttempt = 0;
  private shouldReconnect = false;
  private hasConnectedOnce = false;
  private options: WsClientOptions;

  constructor(options: WsClientOptions = {}) {
    this.options = options;
  }

  /** Returns the current connection status. */
  getStatus(): ConnectionStatus {
    return this.status;
  }

  /** Opens a WebSocket connection to the given URL. */
  connect(url: string): void {
    this.url = url;
    this.shouldReconnect = true;
    this.reconnectAttempt = 0;
    this.openSocket();
  }

  /** Closes the connection and stops reconnection attempts. */
  disconnect(): void {
    this.shouldReconnect = false;
    this.clearReconnectTimer();
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
    this.setStatus("disconnected");
  }

  /** Sends a client message over the WebSocket. */
  send(msg: ClientMessage): boolean {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      return false;
    }
    this.ws.send(JSON.stringify(msg));
    return true;
  }

  private openSocket(): void {
    this.setStatus("connecting");
    try {
      this.ws = new WebSocket(this.url);
    } catch {
      this.scheduleReconnect();
      return;
    }

    this.ws.onopen = () => {
      const wasReconnect = this.hasConnectedOnce;
      this.reconnectAttempt = 0;
      this.hasConnectedOnce = true;
      this.setStatus("connected");
      if (wasReconnect) {
        this.options.onReconnect?.();
      }
    };

    this.ws.onmessage = (event: MessageEvent) => {
      try {
        const msg = JSON.parse(event.data as string) as ServerMessage;
        this.options.onMessage?.(msg);
      } catch {
        // Ignore malformed messages.
      }
    };

    this.ws.onclose = () => {
      this.ws = null;
      if (this.shouldReconnect) {
        this.setStatus("disconnected");
        this.scheduleReconnect();
      } else {
        this.setStatus("disconnected");
      }
    };

    this.ws.onerror = () => {
      // onclose will fire after this.
    };
  }

  private scheduleReconnect(): void {
    if (!this.shouldReconnect) return;
    this.clearReconnectTimer();

    const maxDelay = this.options.maxReconnectDelay ?? 30_000;
    const baseDelay = 1000;
    const delay = Math.min(
      baseDelay * Math.pow(2, this.reconnectAttempt),
      maxDelay,
    );
    this.reconnectAttempt++;

    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      if (this.shouldReconnect) {
        this.openSocket();
      }
    }, delay);
  }

  private clearReconnectTimer(): void {
    if (this.reconnectTimer !== null) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
  }

  private setStatus(status: ConnectionStatus): void {
    if (this.status !== status) {
      this.status = status;
      this.options.onStatusChange?.(status);
    }
  }
}
