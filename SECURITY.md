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
