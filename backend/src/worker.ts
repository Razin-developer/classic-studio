import type { AppEnv } from "./types.js";

/**
 * Cloudflare Worker edge proxy. Atlas MongoDB is TCP-only and is therefore
 * accessed by the Node API above, never directly from a Worker.
 */
export default {
  async fetch(request: Request, env: AppEnv): Promise<Response> {
    if (!env.API_PROXY_URL) {
      return Response.json({ error: { code: "PROXY_NOT_CONFIGURED", message: "Configure API_PROXY_URL as a Worker secret or variable." } }, { status: 503 });
    }
    const incoming = new URL(request.url);
    const target = new URL(incoming.pathname + incoming.search, env.API_PROXY_URL);
    const headers = new Headers(request.headers);
    headers.delete("host");
    headers.set("X-Forwarded-Host", incoming.host);
    return fetch(target, { method: request.method, headers, body: request.body, redirect: "manual" });
  }
} satisfies ExportedHandler<AppEnv>;
