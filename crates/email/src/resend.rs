use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum EmailError {
    #[error("HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("Resend API error: {0}")]
    Api(String),
    #[error("Template error: {0}")]
    Template(String),
    #[error("Configuration error: {0}")]
    Config(String),
}

#[derive(Debug, Clone)]
pub struct EmailService {
    client: Client,
    api_key: String,
    from_email: String,
    from_name: String,
    base_url: String,
}

#[derive(Debug, Serialize)]
struct SendEmailRequest {
    from: String,
    to: Vec<String>,
    subject: String,
    html: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reply_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<Vec<EmailTag>>,
}

#[derive(Debug, Serialize)]
struct EmailTag {
    name: String,
    value: String,
}

#[derive(Debug, Deserialize)]
struct SendEmailResponse {
    id: String,
}

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    message: String,
}

impl EmailService {
    pub fn new(api_key: String, from_email: String, from_name: String) -> Result<Self, EmailError> {
        if api_key.is_empty() {
            return Err(EmailError::Config("RESEND_API_KEY is required".into()));
        }

        Ok(Self {
            client: Client::new(),
            api_key,
            from_email,
            from_name,
            base_url: "https://api.resend.com".into(),
        })
    }

    pub fn from_env() -> Result<Self, EmailError> {
        let api_key = std::env::var("RESEND_API_KEY")
            .map_err(|_| EmailError::Config("RESEND_API_KEY not set".into()))?;
        let from_email = std::env::var("EMAIL_FROM")
            .unwrap_or_else(|_| "noreply@example.com".into());
        let from_name = std::env::var("EMAIL_FROM_NAME")
            .unwrap_or_else(|_| "MCP Cloud".into());

        Self::new(api_key, from_email, from_name)
    }

    fn from_address(&self) -> String {
        format!("{} <{}>", self.from_name, self.from_email)
    }

    pub async fn send_email(
        &self,
        to: &str,
        subject: &str,
        html: &str,
        tags: Option<Vec<(&str, &str)>>,
    ) -> Result<String, EmailError> {
        let tags = tags.map(|t| {
            t.into_iter()
                .map(|(name, value)| EmailTag {
                    name: name.into(),
                    value: value.into(),
                })
                .collect()
        });

        let request = SendEmailRequest {
            from: self.from_address(),
            to: vec![to.into()],
            subject: subject.into(),
            html: html.into(),
            text: None,
            reply_to: None,
            tags,
        };

        let response = self
            .client
            .post(format!("{}/emails", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await?;

        if response.status().is_success() {
            let result: SendEmailResponse = response.json().await?;
            tracing::info!(email_id = %result.id, to = %to, subject = %subject, "Email sent successfully");
            Ok(result.id)
        } else {
            let error: ErrorResponse = response.json().await?;
            tracing::error!(error = %error.message, to = %to, "Failed to send email");
            Err(EmailError::Api(error.message))
        }
    }

    // ========================================
    // High-level email methods
    // ========================================

    /// Send team invitation email
    pub async fn send_team_invite(
        &self,
        to: &str,
        inviter_name: &str,
        workspace_name: &str,
        invite_url: &str,
    ) -> Result<String, EmailError> {
        let html = crate::templates::team_invite(inviter_name, workspace_name, invite_url)?;
        self.send_email(
            to,
            &format!("{} invited you to join {} on MCP Cloud", inviter_name, workspace_name),
            &html,
            Some(vec![("category", "team_invite")]),
        )
        .await
    }

    /// Send deploy success notification
    pub async fn send_deploy_success(
        &self,
        to: &str,
        server_name: &str,
        deploy_url: &str,
    ) -> Result<String, EmailError> {
        let html = crate::templates::deploy_success(server_name, deploy_url)?;
        self.send_email(
            to,
            &format!("Deploy succeeded: {}", server_name),
            &html,
            Some(vec![("category", "deploy_success")]),
        )
        .await
    }

    /// Send deploy failure notification
    pub async fn send_deploy_failure(
        &self,
        to: &str,
        server_name: &str,
        error_message: &str,
        logs_url: &str,
    ) -> Result<String, EmailError> {
        let html = crate::templates::deploy_failure(server_name, error_message, logs_url)?;
        self.send_email(
            to,
            &format!("Deploy failed: {}", server_name),
            &html,
            Some(vec![("category", "deploy_failure")]),
        )
        .await
    }

    /// Send server down alert
    pub async fn send_server_down(
        &self,
        to: &str,
        server_name: &str,
        downtime: &str,
        dashboard_url: &str,
    ) -> Result<String, EmailError> {
        let html = crate::templates::server_down(server_name, downtime, dashboard_url)?;
        self.send_email(
            to,
            &format!("Alert: {} is down", server_name),
            &html,
            Some(vec![("category", "server_down")]),
        )
        .await
    }

    /// Send weekly report
    pub async fn send_weekly_report(
        &self,
        to: &str,
        stats: &crate::templates::WeeklyReportStats,
    ) -> Result<String, EmailError> {
        let html = crate::templates::weekly_report(stats)?;
        self.send_email(
            to,
            "Your weekly MCP Cloud report",
            &html,
            Some(vec![("category", "weekly_report")]),
        )
        .await
    }

    /// Send contact form notification to admin
    pub async fn send_contact_notification(
        &self,
        admin_email: &str,
        sender_name: &str,
        sender_email: &str,
        message: &str,
    ) -> Result<String, EmailError> {
        let html = crate::templates::contact_notification(sender_name, sender_email, message)?;
        self.send_email(
            admin_email,
            &format!("New contact from {}", sender_name),
            &html,
            Some(vec![("category", "contact")]),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_address() {
        let service = EmailService::new(
            "test_key".into(),
            "test@example.com".into(),
            "Test".into(),
        )
        .unwrap();
        assert_eq!(service.from_address(), "Test <test@example.com>");
    }
}
