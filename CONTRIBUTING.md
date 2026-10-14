# Contributing to Classic Studio

Classic Studio is an independent Rust project. Contributions should improve its own document format, implementation, tests, accessibility, and user experience without using proprietary source code, private assets, or copied product UI.

## Contribution areas

- Native canvas, document model, import/export, accessibility, and performance work.
- `.classic` file-format specification and migration fixtures.
- Hosted collaboration protocol, authorization, operational security, and observability.
- Extension-host and local MCP contracts.
- Website documentation, tutorials, free tools, legal copy, and release automation.

## Working agreement

1. Propose significant architecture changes in an issue or design note before implementation.
2. Keep document changes backward-readable or provide an explicit migration.
3. Keep credentials, tokens, connection strings, and personal data out of commits, fixtures, logs, and screenshots.
4. Treat uploads, extension messages, collaboration operations, and model output as untrusted input.
5. Document user-visible behavior and limitations alongside the change.
6. Make generated artifacts reproducible where possible and do not commit local build output.

## Security reporting

Do not open a public issue for a suspected vulnerability. Until a dedicated reporting address is published, contact the maintainers privately through the repository's configured security channel. Rotate any credential that has been posted in a chat, issue, commit, or log before work continues.

## Pull requests

Keep pull requests focused. Include the problem, behavior change, migration impact, and verification performed. Changes affecting file persistence, authentication, authorization, extension permissions, network input, or export should include a threat-model note or linked design note.
