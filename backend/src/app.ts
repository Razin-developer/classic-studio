import { Hono } from "hono";
import { cors } from "hono/cors";
import { secureHeaders } from "hono/secure-headers";
import { z } from "zod";
import { authenticate, hashPassword, issueToken, verifyPassword } from "./lib/auth.js";
import { AppError, errorResponse } from "./lib/errors.js";
import { InMemoryRateLimiter, parseCorsOrigins, sanitizeText } from "./lib/security.js";
import { assertProjectRole, type DataStore } from "./lib/store.js";
import type { AppEnv, AuthUser } from "./types.js";

type Variables = { user: AuthUser };
type Bindings = AppEnv;
type AppDeps = { env: AppEnv; store: DataStore; dashboard?: (data: { projects: number; users: number }) => string };

const credentials = z.object({
  email: z.string().email().max(254).transform((email) => email.toLowerCase()),
  password: z.string().min(12).max(256),
  displayName: z.string().min(1).max(80).transform((name) => sanitizeText(name, 80))
});
const loginInput = credentials.pick({ email: true, password: true });
const projectInput = z.object({ name: z.string().min(1).max(120).transform((name) => sanitizeText(name, 120)) });
const fileInput = z.object({
  name: z.string().min(1).max(180).transform((name) => sanitizeText(name, 180)),
  sizeBytes: z.number().int().min(0).max(500 * 1024 * 1024),
  storageKey: z.string().min(1).max(512),
  extension: z.literal(".classic")
});
const presenceInput = z.object({
  cursor: z.object({ x: z.number().finite(), y: z.number().finite() }).optional()
});

function parse<T>(schema: z.ZodType<T>, value: unknown): T {
  const result = schema.safeParse(value);
  if (!result.success) throw new AppError(400, "The request data is invalid.", "INVALID_REQUEST");
  return result.data;
}

export function createApp({ env, store, dashboard }: AppDeps) {
  const app = new Hono<{ Bindings: Bindings; Variables: Variables }>();
  const rateLimiter = new InMemoryRateLimiter();
  const allowedOrigins = parseCorsOrigins(env.CORS_ORIGINS);

  app.use("*", secureHeaders({ contentSecurityPolicy: { defaultSrc: ["'none'"], frameAncestors: ["'none'"] } }));
  app.use("*", async (c, next) => {
    const client = c.req.header("CF-Connecting-IP") ?? c.req.header("X-Forwarded-For")?.split(",")[0]?.trim() ?? "anonymous";
    const result = rateLimiter.consume(`${client}:${c.req.method}:${new URL(c.req.url).pathname}`, 240, 60_000);
    c.header("RateLimit-Remaining", String(result.remaining));
    c.header("RateLimit-Reset", String(Math.ceil(result.resetAt / 1000)));
    await next();
  });
  app.use("/api/*", cors({ origin: (origin) => allowedOrigins.includes(origin) ? origin : "", allowHeaders: ["Authorization", "Content-Type"], allowMethods: ["GET", "POST", "PUT", "DELETE", "OPTIONS"], maxAge: 86400 }));
  app.onError(errorResponse);

  app.get("/health", (c) => c.json({ status: "ok", service: "classic-studio-api", timestamp: new Date().toISOString() }));
  app.get("/api/v1/capabilities", (c) => c.json({
    collaboration: { transport: "websocket-adapter-required", presence: true },
    storage: { classicFiles: true, binaryUpload: "signed-upload-provider-required" },
    worker: { mongoDirect: false, proxyRequired: true }
  }));

  app.post("/api/v1/auth/register", async (c) => {
    const input = parse(credentials, await c.req.json());
    rateLimiter.consume(`register:${c.req.header("CF-Connecting-IP") ?? "anonymous"}`, 8, 15 * 60_000);
    if (await store.findUserByEmail(input.email)) throw new AppError(409, "An account with that email already exists.", "EMAIL_EXISTS");
    const role = env.ADMIN_EMAIL?.toLowerCase() === input.email ? "admin" : "owner";
    const user = await store.createUser({ email: input.email, displayName: input.displayName, role, passwordHash: await hashPassword(input.password) });
    const { passwordHash: _passwordHash, ...safeUser } = user;
    return c.json({ user: safeUser, token: await issueToken(safeUser, env.JWT_SECRET) }, 201);
  });
  app.post("/api/v1/auth/login", async (c) => {
    const input = parse(loginInput, await c.req.json());
    rateLimiter.consume(`login:${c.req.header("CF-Connecting-IP") ?? "anonymous"}`, 12, 15 * 60_000);
    const user = await store.findUserByEmail(input.email);
    if (!user || !(await verifyPassword(input.password, user.passwordHash))) throw new AppError(401, "Email or password is incorrect.", "INVALID_CREDENTIALS");
    const { passwordHash: _passwordHash, ...safeUser } = user;
    return c.json({ user: safeUser, token: await issueToken(safeUser, env.JWT_SECRET) });
  });

  app.use("/api/v1/projects/*", async (c, next) => {
    c.set("user", await authenticate(c.req.header("Authorization"), env.JWT_SECRET));
    await next();
  });
  app.get("/api/v1/projects", async (c) => c.json({ projects: await store.listProjects(c.get("user")) }));
  app.post("/api/v1/projects", async (c) => {
    const input = parse(projectInput, await c.req.json());
    return c.json({ project: await store.createProject(c.get("user"), input.name) }, 201);
  });
  app.get("/api/v1/projects/:projectId/files", async (c) => {
    const project = await requireProject(c.req.param("projectId"), c.get("user"), store, ["owner", "editor", "viewer", "admin"]);
    return c.json({ files: await store.listFiles(project.id) });
  });
  app.post("/api/v1/projects/:projectId/files", async (c) => {
    const user = c.get("user");
    const project = await requireProject(c.req.param("projectId"), user, store, ["owner", "editor", "admin"]);
    const input = parse(fileInput, await c.req.json());
    return c.json({ file: await store.saveFile(user, { ...input, projectId: project.id }) }, 201);
  });
  app.post("/api/v1/projects/:projectId/comments", async (c) => {
    const project = await requireProject(c.req.param("projectId"), c.get("user"), store, ["owner", "editor", "viewer", "admin"]);
    const input = parse(z.object({ text: z.string().min(1).max(4_000) }), await c.req.json());
    // Persistence belongs in the collaborative document event store; text is sanitized at this boundary.
    return c.json({ projectId: project.id, text: sanitizeText(input.text), acceptedAt: new Date().toISOString() }, 202);
  });

  app.use("/api/v1/collaboration/*", async (c, next) => {
    c.set("user", await authenticate(c.req.header("Authorization"), env.JWT_SECRET));
    await next();
  });
  app.get("/api/v1/collaboration/rooms/:roomId/presence", async (c) => {
    return c.json({ people: await store.listPresence(sanitizeText(c.req.param("roomId"), 128)) });
  });
  app.post("/api/v1/collaboration/rooms/:roomId/presence", async (c) => {
    const input = parse(presenceInput, await c.req.json());
    const user = c.get("user");
    const presence = { roomId: sanitizeText(c.req.param("roomId"), 128), userId: user.id, displayName: user.displayName, cursor: input.cursor, updatedAt: new Date().toISOString() };
    await store.updatePresence(presence);
    return c.json({ presence });
  });
  app.get("/api/v1/collaboration/ws/:roomId", (c) => c.json({
    error: { code: "WEBSOCKET_ADAPTER_REQUIRED", message: "Attach the platform WebSocket adapter to connect this room." },
    roomId: sanitizeText(c.req.param("roomId"), 128)
  }, 501));

  app.get("/admin", async (c) => {
    const user = await authenticate(c.req.header("Authorization"), env.JWT_SECRET);
    if (user.role !== "admin") throw new AppError(403, "Administrator access is required.", "FORBIDDEN");
    const projects = await store.listProjects(user);
    const html = dashboard?.({ projects: projects.length, users: 0 }) ?? "<h1>Classic Studio Admin</h1><p>Dashboard template is unavailable.</p>";
    return c.html(html);
  });
  return app;
}

async function requireProject(id: string, user: AuthUser, store: DataStore, roles: Array<"owner" | "editor" | "viewer" | "admin">) {
  const project = await store.findProject(id);
  if (!project) throw new AppError(404, "The project was not found.", "PROJECT_NOT_FOUND");
  assertProjectRole(project, user, roles);
  return project;
}
