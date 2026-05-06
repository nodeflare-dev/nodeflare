# Security Configuration

## npm Security Settings

This project has strict npm security settings configured in `.npmrc`:

### Settings

| Setting | Value | Purpose |
|---------|-------|---------|
| `ignore-scripts` | `true` | Prevents supply chain attacks via malicious install scripts |
| `audit` | `true` | Enables automatic vulnerability scanning |
| `audit-level` | `high` | Reports high and critical vulnerabilities |
| `strict-ssl` | `true` | Enforces HTTPS for registry connections |
| `save-exact` | `true` | Locks exact versions (no ^ or ~) |
| `package-lock` | `true` | Requires package-lock.json |
| `strict-peer-deps` | `true` | Fails on peer dependency conflicts |
| `engine-strict` | `true` | Enforces Node.js version requirements |

## Installation

```bash
# Standard install (scripts disabled for security)
npm install

# Rebuild native packages that need postinstall
npm run setup

# OR: Install with scripts enabled (less secure)
npm install --ignore-scripts=false
```

## Native Packages

The following packages require `postinstall` scripts:

- `unrs-resolver` - Native binary for fast path resolution

## Known Vulnerabilities

### Accepted Risks (Next.js 14.x)

These vulnerabilities require Next.js 16.x to fix (breaking change):

| CVE | Severity | Condition | Mitigation |
|-----|----------|-----------|------------|
| GHSA-9g9p-9gw9-jx7f | High | self-hosted + remotePatterns | Don't use `remotePatterns` |
| GHSA-h25m-26qc-wcjf | High | insecure RSC | Use secure RSC practices |
| GHSA-ggv3-7p47-pfv8 | High | rewrites | Careful with rewrite rules |
| GHSA-3x4c-7xq6-9pq8 | High | self-hosted | Monitor disk usage |

## Security Checklist

- [ ] Run `npm audit` before deploying
- [ ] Use `npm ci` in CI/CD (strict lockfile install)
- [ ] Review new dependencies before adding
- [ ] Keep dependencies updated regularly
- [ ] Monitor for new CVEs

## Commands

```bash
# Check for vulnerabilities
npm audit

# Fix auto-fixable vulnerabilities
npm audit fix

# List outdated packages
npm outdated

# Check for unused dependencies
npx depcheck
```

## Node.js Version

This project requires Node.js 20.x. Version is pinned in:
- `.nvmrc` - For nvm users
- `package.json` - engines field

```bash
# Use correct Node version with nvm
nvm use
```
