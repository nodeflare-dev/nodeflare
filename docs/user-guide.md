# NodeFlare User Guide

NodeFlare is a hosting service that makes it easy to deploy and manage MCP servers.
This guide explains how to use each screen.

---

## Table of Contents

1. [Dashboard (Home)](#1-dashboard-home)
2. [Server List](#2-server-list)
3. [Create Server](#3-create-server)
4. [Server Details](#4-server-details)
   - [Overview Tab](#41-overview-tab)
   - [Deployments Tab](#42-deployments-tab)
   - [Test Tab](#43-test-tab)
   - [Secrets Tab](#44-secrets-tab)
   - [Webhooks Tab](#45-webhooks-tab)
   - [Metrics Tab](#46-metrics-tab)
   - [Settings Tab](#47-settings-tab)
5. [Logs](#5-logs)
6. [Authentication](#6-authentication)
   - [Access Tokens](#61-access-tokens)
   - [OAuth Apps](#62-oauth-apps)
7. [Team](#7-team)
8. [Billing](#8-billing)
9. [User Settings](#9-user-settings)

---

## 1. Dashboard (Home)

![Dashboard](./screenshots/dashboard.png)

### Overview
The first screen displayed after login. View server status and announcements at a glance.

### Screen Components

| Element | Description |
|---------|-------------|
| Server Cards | View each server's status (Running/Stopped/Error) at a glance |
| Quick Actions | Restart or deploy servers with one click |
| Usage | Current month's request count, server count, etc. |
| Announcements | Important announcements from NodeFlare |

### Actions
- **Click server card** → Navigate to server details
- **"New Server" button** → Navigate to server creation
- **Restart icon** → Restart the server

---

## 2. Server List

![Server List](./screenshots/servers.png)

### Overview
Displays all created MCP servers in a list.

### Screen Components

| Element | Description |
|---------|-------------|
| Server Name | Click to navigate to details |
| Status Badge | Running (green) / Stopped (gray) / Error (red) / Building (yellow) |
| Last Deploy | Most recent deployment date/time |
| Action Buttons | Quick actions like restart, stop, deploy |

### Actions
- **"New Server" button** → Navigate to server creation
- **Click server row** → Navigate to server details
- **Workspace switcher** → Switch between workspaces using the selector at the top

---

## 3. Create Server

![Create Server](./screenshots/server-new.png)

### Overview
Screen for creating a new MCP server. Select a GitHub repository to deploy.

### Input Fields

| Field | Required | Description |
|-------|----------|-------------|
| Server Name | Required | Display name for management (Japanese OK) |
| Slug | Required | Identifier used in URL (lowercase letters and hyphens only)<br>Example: `my-server` → `my-server.nodeflare.tech` |
| GitHub Repository | Required | Select the repository to deploy from |
| Branch | Optional | Branch to deploy (default: main) |
| Root Directory | Optional | For monorepos, the directory containing server code |

### Steps
1. Enter server name
2. Enter slug (auto-generation available)
3. Click "Select Repository" to choose a GitHub repository
4. Change branch if needed
5. Click "Create Server"
6. Build and deployment will start automatically

### Supported Languages
- TypeScript / JavaScript (detects `package.json`)
- Python (detects `requirements.txt` or `pyproject.toml`)
- Go (detects `go.mod`)
- Rust (detects `Cargo.toml`)
- Docker (detects `Dockerfile`)

---

## 4. Server Details

The details screen displayed when clicking a server name.
Features are organized across multiple tabs.

### 4.1 Overview Tab

![Server Overview](./screenshots/server-overview.png)

#### Overview
Displays basic server information and current status.

#### Display Content
- **Status**: Current running state
- **Endpoint URL**: URL for connecting from AI clients
- **Last Deploy**: Most recent deployment date/time and commit hash
- **Available Tools**: List of MCP tools provided by this server

#### Actions
- **"Copy" button** → Copy endpoint URL to clipboard
- **"Deploy" button** → Execute manual deployment
- **"Restart" button** → Restart the server
- **"Stop" button** → Stop the server

---

### 4.2 Deployments Tab

![Deployments](./screenshots/server-deployments.png)

#### Overview
View deployment history and build logs.

#### Display Content

| Item | Description |
|------|-------------|
| Deploy Time | Date/time when deployment was executed |
| Commit | Git commit that was deployed |
| Status | Success / Failed / In Progress |
| Duration | Time taken for build to deployment |

#### Actions
- **Click deployment row** → Expand to show build logs
- **"Rollback" button** → Revert to a previous version

---

### 4.3 Test Tab

![Test](./screenshots/server-test.png)

#### Overview
Test your deployed MCP server directly in the browser.

#### Usage
1. Select a tool on the left
2. Enter required parameters
3. Click "Execute" button
4. Results appear on the right

#### Notes
- Can test without access token (runs with owner permissions)
- Resources and prompts can also be tested

---

### 4.4 Secrets Tab

![Secrets](./screenshots/server-secrets.png)

#### Overview
Store API keys and connection information as encrypted environment variables.

#### Usage
1. Click "Add Secret"
2. Enter key name (e.g., `NOTION_API_KEY`)
3. Enter value
4. Click "Save"

#### Display Content
- Only key names are shown for registered secrets
- Values are masked as `••••••••`

#### Notes
- Once saved, values cannot be displayed (overwrite only)
- Available as environment variables via `process.env.KEY_NAME`
- Redeployment required after changing secrets

---

### 4.5 Webhooks Tab

![Webhooks](./screenshots/server-webhooks.png)

#### Overview
Configure and view GitHub webhooks.
Used to trigger automatic deployments when code is pushed.

#### Display Content
- **Webhook URL**: URL to configure in GitHub
- **Secret**: Secret for webhook verification
- **Recent Deliveries**: Webhook delivery history

#### Setup Instructions
1. Copy Webhook URL
2. Go to GitHub repository Settings → Webhooks
3. Click "Add webhook"
4. Set URL and secret
5. Select `application/json` for Content type
6. Select "push" event

---

### 4.6 Metrics Tab

![Metrics](./screenshots/server-metrics.png)

#### Overview
Visualize server usage with graphs.

#### Display Content
- **Request Count**: Request count graph by time period
- **Response Time**: Average response time trends
- **Error Rate**: Percentage of error responses
- **Tool Usage**: Call count for each tool

#### Time Range
- Switch between 1 hour / 24 hours / 7 days / 30 days

---

### 4.7 Settings Tab

![Server Settings](./screenshots/server-settings.png)

#### Overview
Modify server settings.

#### Settings

| Item | Description |
|------|-------------|
| Server Name | Change display name |
| Slug | Change URL identifier (may break existing connections) |
| Branch | Change deployment target branch |
| Auto Deploy | ON/OFF for automatic deployment on push |
| Region | Select deployment region |

#### Dangerous Actions
- **Delete Server**: Permanently delete server and all data

---

## 5. Logs

![Logs](./screenshots/logs.png)

### Overview
View MCP server request logs in real-time.

### Screen Components

| Element | Description |
|---------|-------------|
| Server Select | Switch which server to display |
| Search | Filter by tool name or response |
| Live Mode | Toggle auto-refresh ON/OFF (2-second interval) |
| Timeline | Visualize request time distribution |

### Log Fields

| Field | Description |
|-------|-------------|
| Time | Date/time request was received |
| Status | HTTP status code |
| Duration | Time taken to process request |
| Tool Name | Name of tool called |

### Actions
- **Live button** → Toggle auto-refresh ON/OFF
- **Refresh button** → Manually refresh logs
- **Download button** → Export logs as PDF

### Plan Limitations
| Plan | Retention Period |
|------|------------------|
| Free | 1 hour |
| Pro | 7 days |
| Team | 30 days |
| Enterprise | 90 days |

---

## 6. Authentication

Authentication features to control access to MCP servers.

### 6.1 Access Tokens

![Access Tokens](./screenshots/access-tokens.png)

#### Overview
Manage tokens used to access MCP servers.

#### Creating a Token
1. Click "New Token" button
2. Enter token name (for identification)
3. Select target server (all servers or specific server)
4. Select scopes
   - **Full Access**: Allow all operations
   - **Tools**: Allow only tool calls
   - **Resources**: Allow only resource reading
   - **Prompts**: Allow only prompt reading
5. Click "Create"
6. Copy the displayed token and store it securely

#### Notes
- Token is displayed only once when created
- If lost, create a new token
- Delete unused tokens

#### Token Usage
```
Authorization: Bearer nf_xxxxxxxxxxxxxxxxxxxx
```

---

### 6.2 OAuth Apps

![OAuth Apps](./screenshots/oauth-apps.png)

#### Overview
Manage OAuth clients for external applications to access MCP servers.

#### Creating an OAuth Client
1. Click "New Client" button
2. Enter application name
3. Enter redirect URIs (multiple allowed)
4. Select allowed scopes
5. Click "Create"
6. Client ID and Client Secret will be issued

#### Issued Credentials

| Item | Description |
|------|-------------|
| Client ID | Used in OAuth authorization requests |
| Client Secret | Used for authentication at token endpoint |

#### Supported Flows
- Authorization Code + PKCE (recommended)
- Client Credentials (for server-to-server communication)

---

## 7. Team

![Team](./screenshots/team.png)

### Overview
Invite members to your workspace to collaboratively manage servers.

### Screen Components
- **Member Count**: Current member count and limit
- **Member List**: Each member's name, email, and role

### Adding Members
1. Click "Add Member" button
2. Enter the email address of the user to invite
3. Select role
4. Click "Invite"

### Roles

| Role | Permissions |
|------|-------------|
| Owner | All operations. Can delete workspace and transfer ownership |
| Admin | Can manage servers and members |
| Member | Can create and edit servers |
| Viewer | View servers only |

### Actions
- **Change Role**: Change via dropdown (except for owner)
- **Remove Member**: Click "Delete" button

### Member Limits by Plan

| Plan | Limit |
|------|-------|
| Free | 1 |
| Pro | 3 |
| Team | 10 |
| Enterprise | Unlimited |

---

## 8. Billing

![Billing](./screenshots/billing.png)

### Overview
Manage subscriptions and view billing information.

### Screen Components

#### Current Plan
- Plan name and price
- Next billing date
- Usage (server count, request count, etc.)

#### Change Plan
- Upgrade/downgrade via "Change Plan" button
- Upgrades take effect immediately
- Downgrades apply from next billing date

#### Payment Methods
- Register/change credit card
- Supported cards: Visa, Mastercard, American Express, JCB

#### Billing History
- List of past invoices
- Download PDF receipts for each invoice

### Plan Comparison

| Feature | Free | Pro | Team | Enterprise |
|---------|------|-----|------|------------|
| Monthly Price | $0 | $29 | $99 | Contact Us |
| Servers | 1 | 5 | 20 | Unlimited |
| Team Members | 1 | 3 | 10 | Unlimited |
| Requests/Month | 1,000 | 50,000 | 500,000 | Unlimited |
| Log Retention | 1 hour | 7 days | 30 days | 90 days |
| Custom Domain | - | - | Yes | Yes |
| Priority Support | - | - | Yes | Yes |

---

## 9. User Settings

![User Settings](./screenshots/settings.png)

### Overview
Manage account settings and notification preferences.

### Profile Settings

| Item | Description |
|------|-------------|
| Display Name | Name shown in dashboard and team |
| Email | Notification destination |
| Language | Japanese / English |

### Notification Settings

| Notification | Description |
|--------------|-------------|
| Deploy Success | Email notification when deployment completes |
| Deploy Failed | Email notification when deployment fails (recommended: ON) |
| Server Down | Email notification when server stops (recommended: ON) |
| Weekly Report | Weekly usage summary |

### Other
- **Unlink GitHub Account**: Not currently supported
- **Delete Account**: Deletes all data permanently

---

## Screenshot List

List of screenshot filenames used in the documentation.
Place them in the `public/screenshots/` folder.

| Filename | Screen |
|----------|--------|
| `dashboard.png` | Dashboard (Home) |
| `servers.png` | Server List |
| `server-new.png` | Create Server |
| `server-overview.png` | Server Details - Overview Tab |
| `server-deployments.png` | Server Details - Deployments Tab |
| `server-test.png` | Server Details - Test Tab |
| `server-secrets.png` | Server Details - Secrets Tab |
| `server-webhooks.png` | Server Details - Webhooks Tab |
| `server-metrics.png` | Server Details - Metrics Tab |
| `server-settings.png` | Server Details - Settings Tab |
| `logs.png` | Logs |
| `access-tokens.png` | Access Tokens |
| `oauth-apps.png` | OAuth Apps |
| `team.png` | Team |
| `billing.png` | Billing |
| `settings.png` | User Settings |
