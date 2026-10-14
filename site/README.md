# Classic Studio site

This directory is a dependency-free static marketing and documentation site for Classic Studio. It is intentionally an original product presentation; it does not reproduce another product's interface, markup, or assets.

## Local preview

Open `index.html` directly in a browser, or serve this directory with any static server. No build step is required.

### Hono / Cloudflare Workers

The site can be mounted as static assets from a Hono Worker, for example:

```ts
import { Hono } from 'hono';
import { serveStatic } from 'hono/cloudflare-workers';

const app = new Hono();
app.use('/*', serveStatic({ root: './site' }));
export default app;
```

Configure the Worker’s static-assets binding according to the current Wrangler documentation. Keep project configuration, runtime secrets, authentication, rate limiting, and database access outside this directory.

## Release URLs

Download buttons are deliberately inactive until verified releases exist. Replace them only after a release workflow publishes platform artifacts, checksums, and provenance. The source repository links can be changed in the HTML or by setting `window.CLASSIC_GITHUB_URL` before `app.js` runs.

Before production, update the canonical URL, Open Graph image, legal contact details, privacy notice, terms, and security reporting channel. The legal pages are project drafts, not legal advice.

## Pages

- `index.html` — product landing page
- `features.html` — current editor scope
- `downloads.html` — release and build guidance
- `roadmap.html` — accessible green/yellow/gray delivery status
- `blog.html`, `tools.html` — publishing stubs
- `legal/` — privacy, terms, and security drafts
