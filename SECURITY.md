# Security Policy

## Reporting a Vulnerability

We take security issues seriously. If you discover a vulnerability in Meetdy, please report it responsibly.

**Do not open a public GitHub issue.** Instead, email `security@meetdy.app` with:

- A clear description of the vulnerability
- Steps to reproduce
- Affected versions
- Any potential mitigations you've identified

We will acknowledge your report within 48 hours and provide an expected timeline for a fix.

## Supported Versions

| Version | Supported |
|---------|-----------|
| v0.7.x  | ✅ |
| v0.6.x  | ❌ (pre-release) |

## Scope

Meetdy is a desktop application that:
- Records audio from your microphone
- Processes speech locally via on-device Whisper/Parakeet models
- Outputs transcribed text to the system clipboard

**In scope:**
- Remote code execution via crafted input
- Local privilege escalation
- Sensitive data (audio/text) exfiltration
- Bypass of Tauri CSP or sandbox restrictions
- Dependency vulnerabilities in Rust crates or npm packages

**Out of scope:**
- Social engineering attacks
- Physical access attacks
- Denial of service via resource exhaustion (unless a clear code-level flaw)

## Disclosure

We follow coordinated disclosure. After a fix is released (with an advisory), we will credit reporters (unless you prefer anonymity).
