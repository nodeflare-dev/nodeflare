use crate::models::{NotificationSettings, UpdateNotificationSettings};
use mcp_common::Result;
use sqlx::PgPool;
use uuid::Uuid;

pub struct NotificationSettingsRepository;

impl NotificationSettingsRepository {
    pub async fn get_or_create(pool: &PgPool, user_id: Uuid) -> Result<NotificationSettings> {
        let settings = sqlx::query_as::<_, NotificationSettings>(
            r#"
            INSERT INTO notification_settings (user_id)
            VALUES ($1)
            ON CONFLICT (user_id) DO UPDATE SET user_id = notification_settings.user_id
            RETURNING user_id, email_deploy_success, email_deploy_failure, email_server_down, email_weekly_report, created_at, updated_at
            "#,
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        Ok(settings)
    }

    pub async fn update(
        pool: &PgPool,
        user_id: Uuid,
        data: UpdateNotificationSettings,
    ) -> Result<NotificationSettings> {
        let settings = sqlx::query_as::<_, NotificationSettings>(
            r#"
            INSERT INTO notification_settings (user_id, email_deploy_success, email_deploy_failure, email_server_down, email_weekly_report)
            VALUES ($1, COALESCE($2, true), COALESCE($3, true), COALESCE($4, true), COALESCE($5, false))
            ON CONFLICT (user_id) DO UPDATE SET
                email_deploy_success = COALESCE($2, notification_settings.email_deploy_success),
                email_deploy_failure = COALESCE($3, notification_settings.email_deploy_failure),
                email_server_down = COALESCE($4, notification_settings.email_server_down),
                email_weekly_report = COALESCE($5, notification_settings.email_weekly_report),
                updated_at = NOW()
            RETURNING user_id, email_deploy_success, email_deploy_failure, email_server_down, email_weekly_report, created_at, updated_at
            "#,
        )
        .bind(user_id)
        .bind(data.email_deploy_success)
        .bind(data.email_deploy_failure)
        .bind(data.email_server_down)
        .bind(data.email_weekly_report)
        .fetch_one(pool)
        .await?;

        Ok(settings)
    }
}
