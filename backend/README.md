# Classic Studio API

This is the optional network service for Classic Studio. It is intentionally separate from the Rust desktop editor. The API owns accounts, project and `.classic` file metadata, collaboration presence, and an authenticated Handlebars admin view.

## Security model

- Secrets come only from the process environment or Wrangler secrets. The repository contains no connection strings, credentials, or tokens.
- Passwords use bcrypt with cost 12. JWTs use HS256 and expire after 12 hours. Set a unique `JWT_SECRET` with 32 or more characters.
- Roles are `owner`, `editor`, `viewer`, and `admin`. The user whose email matches `ADMIN_EMAIL` receives `admin` only when they register. Set that environment variable before first registration.
- Hono security headers, explicit CORS origins, request schemas, JSON error envelopes, text sanitization, and per-process rate limits are enabled.
- The included in-memory limiter is suitable for local development and one Node process. Use Durable Objects, Redis, or an edge gateway for a distributed deployment.
- The API accepts `.classic` metadata. Send binary uploads directly to object storage using a signed-upload provider, then register the returned storage key with this API. This keeps API workers out of the large-file data path.

## Local Node deployment with MongoDB Atlas

Atlas' MongoDB driver opens TCP connections. Run this Node target against Atlas; do not place an Atlas URI in a Worker or repository file.

```powershell
cd backend
Copy-Item .env.example .env
# Set the variables in .env through your shell, secret manager, or deployment environment.
npm install
npm run dev
```

`tsx` does not load `.env` automatically. For local development, use your preferred environment loader or set the variables in PowerShell before `npm run dev`.

Validate the project with:

```powershell
npm run typecheck
```

## Cloudflare Workers

The included Worker is an edge proxy to a separately hosted Node API. It cannot make a direct MongoDB Atlas TCP connection.

```powershell
cd backend
npm run deploy:workers
wrangler secret put API_PROXY_URL
wrangler secret put JWT_SECRET
wrangler secret put CORS_ORIGINS
```

The Node service remains the MongoDB adapter. Replace the proxy with an HTTPS-compatible data adapter only after preserving the API's authentication and authorization checks.

## GitHub and release workflow

Initialize and publish only after review:

```powershell
git init
git add backend
git commit -m "Add Classic Studio collaboration API"
gh repo create classic-studio --source . --private --push
```

The commands above are documentation only. They do not run automatically and no credentials are read or transmitted by this project.

## HTTP endpoints

| Endpoint | Purpose |
| --- | --- |
| `GET /health` | Liveness response. |
| `POST /api/v1/auth/register` | Register an account and receive a JWT. |
| `POST /api/v1/auth/login` | Receive a JWT. |
| `GET/POST /api/v1/projects` | List or create accessible projects. |
| `GET/POST /api/v1/projects/:id/files` | List/register `.classic` project-file metadata. |
| `POST /api/v1/projects/:id/comments` | Validates and sanitizes a comment boundary. |
| `GET/POST /api/v1/collaboration/rooms/:id/presence` | Read or refresh live-presence records. |
| `GET /api/v1/collaboration/ws/:id` | Returns the socket-adapter requirement until a Node or Durable Object WebSocket adapter is configured. |
| `GET /admin` | Handlebars admin dashboard; requires an admin bearer token. |

## Realtime adapter

`src/collaboration.ts` defines the adapter contract for real WebSocket upgrades and broadcasts. It supports presence, document operations, and comments. Implement it using a Node WebSocket server, Cloudflare Durable Objects, or a managed broker, then wire it into the authenticated collaboration route. The REST presence routes are already usable for heartbeat and cursor state.
