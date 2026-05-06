use crate::EmailError;
use serde::Serialize;

const BASE_STYLE: &str = r#"
    body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; line-height: 1.6; color: #333; max-width: 600px; margin: 0 auto; padding: 20px; }
    .container { background: #ffffff; border-radius: 8px; padding: 32px; border: 1px solid #e5e5e5; }
    .header { text-align: center; margin-bottom: 24px; }
    .logo { font-size: 24px; font-weight: bold; color: #6366f1; }
    .content { margin-bottom: 24px; }
    .button { display: inline-block; background: #6366f1; color: white !important; padding: 12px 24px; text-decoration: none; border-radius: 6px; font-weight: 500; }
    .button:hover { background: #4f46e5; }
    .footer { text-align: center; font-size: 12px; color: #666; margin-top: 32px; padding-top: 16px; border-top: 1px solid #e5e5e5; }
    .alert { background: #fef2f2; border: 1px solid #fecaca; border-radius: 6px; padding: 16px; margin: 16px 0; }
    .alert-title { color: #dc2626; font-weight: 600; margin-bottom: 8px; }
    .success { background: #f0fdf4; border: 1px solid #bbf7d0; }
    .success .alert-title { color: #16a34a; }
    .stat-box { background: #f9fafb; border-radius: 6px; padding: 16px; margin: 8px 0; }
    .stat-value { font-size: 24px; font-weight: bold; color: #6366f1; }
    .stat-label { font-size: 12px; color: #666; text-transform: uppercase; }
    code { background: #f3f4f6; padding: 2px 6px; border-radius: 4px; font-family: monospace; font-size: 14px; }
"#;

fn wrap_template(content: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <style>{}</style>
</head>
<body>
    <div class="container">
        <div class="header">
            <div class="logo">MCP Cloud</div>
        </div>
        <div class="content">
            {}
        </div>
        <div class="footer">
            <p>MCP Cloud - Deploy MCP servers in seconds</p>
            <p><a href="https://mcpcloud.dev">mcpcloud.dev</a></p>
        </div>
    </div>
</body>
</html>"#,
        BASE_STYLE, content
    )
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// Team invitation email template
pub fn team_invite(inviter_name: &str, workspace_name: &str, invite_url: &str) -> Result<String, EmailError> {
    let content = format!(
        r#"
        <h2>You're invited to join a team!</h2>
        <p><strong>{}</strong> has invited you to join <strong>{}</strong> on MCP Cloud.</p>
        <p>MCP Cloud makes it easy to deploy and manage MCP (Model Context Protocol) servers for AI applications.</p>
        <p style="text-align: center; margin: 32px 0;">
            <a href="{}" class="button">Accept Invitation</a>
        </p>
        <p style="font-size: 14px; color: #666;">
            If you don't want to join this team, you can ignore this email.
        </p>
        "#,
        escape_html(inviter_name),
        escape_html(workspace_name),
        escape_html(invite_url)
    );
    Ok(wrap_template(&content))
}

/// Deploy success notification template
pub fn deploy_success(server_name: &str, deploy_url: &str) -> Result<String, EmailError> {
    let content = format!(
        r#"
        <div class="alert success">
            <div class="alert-title">Deploy Successful</div>
            <p>Your server <strong>{}</strong> has been deployed successfully.</p>
        </div>
        <p style="text-align: center; margin: 32px 0;">
            <a href="{}" class="button">View Server</a>
        </p>
        "#,
        escape_html(server_name),
        escape_html(deploy_url)
    );
    Ok(wrap_template(&content))
}

/// Deploy failure notification template
pub fn deploy_failure(server_name: &str, error_message: &str, logs_url: &str) -> Result<String, EmailError> {
    let content = format!(
        r#"
        <div class="alert">
            <div class="alert-title">Deploy Failed</div>
            <p>Your server <strong>{}</strong> failed to deploy.</p>
        </div>
        <h3>Error Details</h3>
        <p><code>{}</code></p>
        <p style="text-align: center; margin: 32px 0;">
            <a href="{}" class="button">View Logs</a>
        </p>
        "#,
        escape_html(server_name),
        escape_html(error_message),
        escape_html(logs_url)
    );
    Ok(wrap_template(&content))
}

/// Server down alert template
pub fn server_down(server_name: &str, downtime: &str, dashboard_url: &str) -> Result<String, EmailError> {
    let content = format!(
        r#"
        <div class="alert">
            <div class="alert-title">Server Down Alert</div>
            <p>Your server <strong>{}</strong> appears to be down.</p>
        </div>
        <p><strong>Downtime:</strong> {}</p>
        <p>Please check your server status and logs for more information.</p>
        <p style="text-align: center; margin: 32px 0;">
            <a href="{}" class="button">Go to Dashboard</a>
        </p>
        "#,
        escape_html(server_name),
        escape_html(downtime),
        escape_html(dashboard_url)
    );
    Ok(wrap_template(&content))
}

/// Weekly report stats
#[derive(Debug, Serialize)]
pub struct WeeklyReportStats {
    pub total_requests: u64,
    pub total_deploys: u64,
    pub uptime_percent: f64,
    pub active_servers: u32,
    pub period_start: String,
    pub period_end: String,
}

/// Weekly report template
pub fn weekly_report(stats: &WeeklyReportStats) -> Result<String, EmailError> {
    let content = format!(
        r#"
        <h2>Your Weekly Report</h2>
        <p style="color: #666;">{} - {}</p>

        <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 16px; margin: 24px 0;">
            <div class="stat-box">
                <div class="stat-value">{}</div>
                <div class="stat-label">Total Requests</div>
            </div>
            <div class="stat-box">
                <div class="stat-value">{}</div>
                <div class="stat-label">Deploys</div>
            </div>
            <div class="stat-box">
                <div class="stat-value">{:.1}%</div>
                <div class="stat-label">Uptime</div>
            </div>
            <div class="stat-box">
                <div class="stat-value">{}</div>
                <div class="stat-label">Active Servers</div>
            </div>
        </div>
        "#,
        escape_html(&stats.period_start),
        escape_html(&stats.period_end),
        stats.total_requests,
        stats.total_deploys,
        stats.uptime_percent,
        stats.active_servers
    );
    Ok(wrap_template(&content))
}

/// Contact form notification template (for admins)
pub fn contact_notification(sender_name: &str, sender_email: &str, message: &str) -> Result<String, EmailError> {
    let content = format!(
        r#"
        <h2>New Contact Message</h2>
        <p><strong>From:</strong> {} &lt;{}&gt;</p>
        <div style="background: #f9fafb; border-radius: 6px; padding: 16px; margin: 16px 0; white-space: pre-wrap;">
{}
        </div>
        <p>
            <a href="mailto:{}">Reply to {}</a>
        </p>
        "#,
        escape_html(sender_name),
        escape_html(sender_email),
        escape_html(message),
        escape_html(sender_email),
        escape_html(sender_name)
    );
    Ok(wrap_template(&content))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_team_invite_template() {
        let html = team_invite("John", "Acme Corp", "https://example.com/invite").unwrap();
        assert!(html.contains("John"));
        assert!(html.contains("Acme Corp"));
        assert!(html.contains("https://example.com/invite"));
    }

    #[test]
    fn test_escape_html() {
        assert_eq!(escape_html("<script>"), "&lt;script&gt;");
    }
}
