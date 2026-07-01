# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in Nodeflare, please report it responsibly.

**Do NOT create a public GitHub issue for security vulnerabilities.**

### How to Report

1. Email: Send details to **sekiguchishunya0619@gmail.com** (or create a private security advisory on GitHub)
2. Include:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Any suggested fixes (optional)

### What to Expect

- **Acknowledgment**: We will acknowledge receipt within 48 hours
- **Updates**: We will provide updates on the progress
- **Resolution**: We aim to resolve critical issues within 7 days
- **Credit**: We will credit reporters in the security advisory (unless you prefer anonymity)

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| Latest  | :white_check_mark: |

## Security Measures

Nodeflare implements the following security measures:

- **Encryption**: AES-256-GCM for secrets at rest
- **Authentication**: JWT with secure signing, OAuth 2.0 support
- **Rate Limiting**: Sliding window rate limiting with Redis
- **Input Validation**: Strict validation on all inputs
- **SQL Injection Prevention**: Parameterized queries via SQLx
- **Sandboxed Builds**: Isolated Docker builds with resource limits

## Best Practices for Users

- Keep your access tokens secure
- Use environment variables for secrets
- Regularly rotate API keys
- Enable OAuth scopes appropriately
- Monitor access logs for suspicious activity
