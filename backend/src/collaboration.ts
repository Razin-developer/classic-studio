import type { AuthUser, Presence } from "./types.js";

export type CollaborationMessage =
  | { type: "presence"; payload: Presence }
  | { type: "document-operation"; payload: { operationId: string; documentId: string; payload: unknown } }
  | { type: "comment"; payload: { threadId: string; text: string } };

/** Platform adapters own the actual socket upgrade and fan-out implementation. */
export interface CollaborationSocketAdapter {
  connect(input: { roomId: string; user: AuthUser; onMessage: (message: CollaborationMessage) => Promise<void> }): Response | Promise<Response>;
  broadcast(roomId: string, message: CollaborationMessage): Promise<void>;
}

/** Implement with Durable Objects, a Node WebSocket server, or a managed broker. */
export class UnconfiguredCollaborationAdapter implements CollaborationSocketAdapter {
  connect(): Response { return Response.json({ error: { code: "WEBSOCKET_ADAPTER_REQUIRED" } }, { status: 501 }); }
  async broadcast(): Promise<void> { throw new Error("A collaboration socket adapter has not been configured."); }
}
