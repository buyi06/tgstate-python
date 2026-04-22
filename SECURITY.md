# Security Policy

## Supported Versions

Security fixes are applied to the `main` branch only. Prior releases are not
maintained.

## Reporting a Vulnerability

Please do **not** open a public GitHub issue for security problems.

Instead, email **security@example.com** with:

- A description of the issue and its impact.
- Steps to reproduce (proof-of-concept preferred).
- Your disclosure timeline preferences.

We aim to acknowledge reports within 3 business days and to ship a fix or
mitigation within 30 days for high-severity issues.

## Scope

In scope:

- The Rust web server and all routes it exposes.
- Authentication, session, and upload/download handlers.
- Database access and Telegram API integration.

Out of scope:

- Issues that require a malicious administrator (the operator has full
  control of the instance by design).
- Social engineering of the bot owner / Telegram channel.
- Third-party dependencies without a proven exploit in this project.

## Hardening tips for operators

- Set `PASS_WORD` so the web UI is not world-editable.
- Put the service behind a reverse proxy that terminates TLS and forwards
  `X-Forwarded-Proto: https`; set `COOKIE_SECURE=1` to force `Secure` cookies
  even when the direct listener is plaintext.
- Only set `TRUST_FORWARDED_FOR=1` if the proxy is trusted to overwrite
  `X-Forwarded-For` / `X-Real-IP`. Otherwise rate limiting can be bypassed.
- Back up `data.db` regularly; it contains the hashed password and session
  token.

## Public endpoints (by design)

The following endpoints do **not** require authentication and are intended to
be reachable by anonymous clients. This is the product's sharing model; do
not file issues against it.

- `GET /d/:short_id` — streams a shared file by its short identifier.
- `GET /share/:short_id` — renders an HTML preview page for the same file.
- `GET /api/health` — used by Docker / load balancers.
- `GET /`, `GET /login`, and `POST /api/auth/login` on first boot (when no
  password has been set).

`short_id` values behave like bearer tokens. They are generated with
cryptographically-random input and only the prefix is ever logged, but if you
need stricter access control you must deploy the service behind an
authenticating reverse proxy. If a short link leaks, delete the underlying
file — there is no way to rotate `short_id` while keeping the stored data.

## Rate limiting

Rate limits are keyed per client IP address and per bucket:

- `login` — guards `/api/auth/login`.
- `upload` — guards `/api/upload`.
- `api` — guards the remaining `/api/*` surface.
- `download` — guards the anonymous `/d/*` and `/share/*` surface.

Behind a reverse proxy you must set `TRUST_FORWARDED_FOR=1` for the limiter
to see real client IPs; otherwise every request appears to come from the
proxy and a single misbehaving client can exhaust the bucket for everyone.
