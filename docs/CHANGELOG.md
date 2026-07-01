# Changelog

All notable changes to Nodeflare will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Multi-region deployment support with automatic Fly.io machine management
- Deploy job queueing for all regions
- Automatic deployment trigger after region checkout completion
- Comprehensive security documentation (OWASP Top 10 compliance)
- Architecture documentation with system diagrams
- Deployment guide for Fly.io production setup
- Contributing guidelines with code style and commit conventions
- Batch stats endpoint for dashboard performance optimization
- Blog integration with Hygraph CMS
- DOMPurify for HTML sanitization in blog content
- GitHub-style team member invitation modal UI
- WireGuard VPN support for secure connections
- Console exec endpoint for remote command execution
- User preferences and notification settings
- System announcements feature
- Contact form endpoint

### Changed
- Enhanced encryption key validation in production environments
- Improved security logging (removed PII from logs)
- Optimized auth session retrieval with Redis caching
- Updated Next.js Image configuration for external domains
- Improved team member management UI

### Security
- Added production environment check for ENCRYPTION_KEY
- Removed email addresses from authentication logs
- Updated SECURITY.md with complete security guidelines
- Added DOMPurify HTML sanitization for CMS content
- Improved nonce generation for encryption

---

## [0.1.0] - 2024-03-29

### Added

#### Core Platform
- MCP server deployment and management platform
- GitHub OAuth authentication with JWT sessions
- Role-based access control (Owner, Admin, Member, Viewer)
- Workspace-based multi-tenancy

#### Server Management
- Create, update, delete MCP servers
- GitHub repository integration
- Multiple runtime support (Node.js, Python, Go, Rust)
- Environment variable management with AES-256-GCM encryption
- API key authentication for servers

#### Deployment
- Automated build pipeline with Docker
- Fly.io Machines deployment
- Build logs and deployment history
- Rollback to previous deployments
- Deploy webhooks with SSRF protection

#### Billing (Stripe Integration)
- Subscription plans (Free, Pro, Team, Enterprise)
- Usage-based billing for additional regions
- Customer portal integration
- Webhook handling for payment events

#### MCP Proxy
- Request routing to MCP servers
- Redis-based response caching
- Request coalescing for concurrent requests
- Prometheus metrics export

#### Frontend (Next.js)
- Dashboard for workspace management
- Server creation and configuration UI
- Deployment logs viewer
- Team member management
- Billing and subscription management
- Multi-language support (i18n)

#### Infrastructure
- PostgreSQL database with SQLx
- Redis caching and job queue
- Rate limiting per IP and user
- WebSocket support for real-time updates

### Security
- CSRF protection with state parameter
- HttpOnly secure cookies
- Parameterized SQL queries (no injection)
- Constant-time API key verification
- Brute force protection with exponential backoff
- SSRF protection for webhooks

---

## Version History

| Version | Date | Highlights |
|---------|------|------------|
| 0.1.0 | 2024-03-29 | Initial release with core features |

---

## Migration Notes

### Upgrading to 0.1.0

This is the initial release. No migration required.

### Future Migrations

Migration notes will be added here for breaking changes.

---

## Contributors

Thanks to all contributors who helped build Nodeflare!

<!-- Contributors will be listed here -->
