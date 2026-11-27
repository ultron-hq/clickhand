# Security policy

Ultron controls a desktop: it moves the mouse, types, reads the screen and runs code it wrote
itself. We treat security reports as the top priority.

## Reporting a vulnerability

Please **do not open a public issue**. Report privately through GitHub:
[Security → Report a vulnerability](https://github.com/ultron-hq/openhud/security/advisories/new).

Include what you found, how to reproduce it, and what an attacker could do with it. We aim to
reply within 72 hours and to ship a fix or mitigation within 30 days, and we will credit you in
the advisory unless you prefer otherwise.

## In scope

- Text on screen, in web pages or in files that makes Ultron act without the user asking (prompt injection into the desktop agent).
- Escapes from the ability sandbox, or abilities getting permissions they were not granted.
- The kill switch failing to stop desktop control.
- Secrets (API keys, tokens) written in plain text, logged, or sent anywhere but their provider.
- Local IPC that lets another user on the machine control Ultron.

## Supported versions

Until 1.0, only the latest release and `main` receive fixes.
