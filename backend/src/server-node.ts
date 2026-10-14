import { serve } from "@hono/node-server";
import Handlebars from "handlebars";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { createApp } from "./app.js";
import { MongoStore } from "./lib/store.js";
import type { AppEnv } from "./types.js";

const env: AppEnv = {
  MONGODB_URI: process.env.MONGODB_URI,
  MONGODB_DB: process.env.MONGODB_DB,
  JWT_SECRET: process.env.JWT_SECRET,
  ADMIN_EMAIL: process.env.ADMIN_EMAIL,
  CORS_ORIGINS: process.env.CORS_ORIGINS,
  APP_ENV: process.env.APP_ENV
};

if (!env.MONGODB_URI) throw new Error("MONGODB_URI is required for the Node API.");
if (!env.JWT_SECRET || env.JWT_SECRET.length < 32) throw new Error("JWT_SECRET must be at least 32 characters.");

const templatePath = fileURLToPath(new URL("../templates/admin.hbs", import.meta.url));
const renderDashboard = Handlebars.compile(readFileSync(templatePath, "utf8"));
const store = await MongoStore.connect(env.MONGODB_URI, env.MONGODB_DB);
const app = createApp({ env, store, dashboard: renderDashboard });
const port = Number(process.env.PORT ?? 8787);

const server = serve({ fetch: app.fetch, port });
console.info(`Classic Studio API listening on :${port}`);

async function shutdown(signal: string) {
  console.info(`Received ${signal}; closing database connection.`);
  server.close();
  await store.close();
  process.exit(0);
}
process.once("SIGINT", () => void shutdown("SIGINT"));
process.once("SIGTERM", () => void shutdown("SIGTERM"));
