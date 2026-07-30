// Typed JSON-RPC 2.0 client over WebSocket. Request/response is correlated
// by id; server-push notifications (poll.update, connection.status) land in
// the subscription callbacks wired in a later increment.
//
// The RPC types are generated from Rust by ts-rs (see ../src/types), so the
// frontend contract cannot drift from the backend.

import type { ConnectionConfig } from "./types/ConnectionConfig";
import type { ConnectionId } from "./types/ConnectionId";
import type { ReadHoldingRegistersRequest } from "./types/ReadHoldingRegistersRequest";
import type { CloseRequest } from "./types/CloseRequest";

interface JsonRpcResponse {
  jsonrpc: "2.0";
  id: number;
  result?: unknown;
  error?: { code: number; message: string };
}

function wsUrl(): string {
  const proto = window.location.protocol === "https:" ? "wss:" : "ws:";
  return `${proto}//${window.location.host}/ws`;
}

export class RpcClient {
  private ws: WebSocket;
  private nextId = 1;
  private pending = new Map<number, (r: JsonRpcResponse) => void>();

  constructor() {
    this.ws = new WebSocket(wsUrl());
    this.ws.onmessage = (ev) => {
      const msg = JSON.parse(ev.data as string) as JsonRpcResponse;
      const resolve = this.pending.get(msg.id);
      if (resolve) {
        this.pending.delete(msg.id);
        resolve(msg);
      }
    };
  }

  ready(): Promise<void> {
    return new Promise((resolve, reject) => {
      if (this.ws.readyState === WebSocket.OPEN) return resolve();
      this.ws.onopen = () => resolve();
      this.ws.onerror = () => reject(new Error("WebSocket connection failed"));
    });
  }

  private call<T>(method: string, params: unknown): Promise<T> {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      this.pending.set(id, (r) =>
        r.error ? reject(new Error(`${r.error.code}: ${r.error.message}`)) : resolve(r.result as T),
      );
      this.ws.send(JSON.stringify({ jsonrpc: "2.0", id, method, params }));
    });
  }

  createConnection(config: ConnectionConfig): Promise<ConnectionId> {
    return this.call<ConnectionId>("connection.create", config);
  }

  closeConnection(req: CloseRequest): Promise<null> {
    return this.call<null>("connection.close", req);
  }

  readHoldingRegisters(req: ReadHoldingRegistersRequest): Promise<number[]> {
    return this.call<number[]>("read.holdingRegisters", req);
  }
}
