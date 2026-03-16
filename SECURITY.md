# Security Policy

## Reporting a Vulnerability

Please report security vulnerabilities through GitHub's private vulnerability reporting feature:

1. Go to the repository's **Security** tab
2. Click **Report a vulnerability**
3. Provide a description, reproduction steps, and impact assessment

Do **not** open a public issue for security vulnerabilities.

## Response Timeline

- **Acknowledgment**: within 48 hours
- **Initial assessment**: within 7 days
- **Fix or mitigation**: best effort, depending on severity

## Scope

In scope:

- Authentication or authorization bypass
- Data exposure (unauthorized access to documents, assets, or user data)
- Server-side injection (SQL, command, path traversal)
- Cross-site scripting (XSS) or cross-site request forgery (CSRF)
- Secrets or credentials leaked in source code

Out of scope:

- Denial of service via resource exhaustion (e.g., uploading very large files)
- Social engineering
- Issues in dependencies without a demonstrated exploit path

## Supported Versions

Only the latest release on the `main` branch is supported with security fixes.
