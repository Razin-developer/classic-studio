# Release and deployment strategy

The current repository contains the native desktop editor. Hosted services, web pages, and automated multi-platform releases are planned work. This document defines the target pipeline without implying that a production deployment exists today.

## GitHub Actions plan

| Stage | Trigger | Target outcome |
|---|---|---|
| Formatting and checks | Pull request | Rust formatting, workspace checks, documentation link checks, and dependency review |
| Security checks | Pull request and scheduled | Advisory scan, secret scan, license policy, and lockfile review |
| Windows build | Pull request and release tag | Signed or unsigned Windows artifact according to configured credentials |
| macOS build | Pull request and release tag | Universal or per-architecture package, notarized only when credentials are configured |
| Linux build | Pull request and release tag | AppImage, package, or archive with checksums |
| Release publication | Protected version tag | GitHub Release notes, checksums, SBOM, and provenance where supported |
| Website deploy | Protected website change | Preview for pull requests and production deployment after review |
| Hosted API deploy | Protected service change | Staged worker deployment with migration and smoke-check gates |

Release tags should be immutable and created only after the build matrix succeeds. The workflow should generate SHA-256 checksums and attach a software bill of materials for every downloadable artifact. Code signing and notarization must use provider-managed secrets; they must never be committed to the repository.

## Hosted service plan

The planned Hono service should run through Cloudflare Workers or a compatible runtime, with separate development, staging, and production environments. Configuration belongs in environment bindings and secret storage. MongoDB or another persistence service must be accessed through a least-privilege service account and a server-side secret only.

Before public collaboration, the service needs schema migrations, authenticated sessions, authorization checks on every project operation, rate limiting, request-size caps, structured security logging, monitoring, backup and restore drills, and an incident response path.

## Landing site plan

The public site should provide verified download links, checksums, platform requirements, documentation, privacy and terms pages, a security page, release notes, project examples, blog posts, and contributor links. Downloads must come from the signed release pipeline rather than ad-hoc file uploads.

## Release gates

1. All supported desktop builds complete successfully.
2. File-format fixtures and migrations load successfully.
3. Security and secret scans pass.
4. Website and API deploys pass their staging checks.
5. A maintainer reviews release notes, platform artifacts, checksums, and rollback instructions.
