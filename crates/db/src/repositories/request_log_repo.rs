use crate::models::{CreateRequestLog, RequestLog, RequestLogStats, RequestLogWithCount, ToolUsageStats};
use chrono::{DateTime, Utc};
use mcp_common::Result;
use sqlx::PgPool;
use uuid::Uuid;

pub struct RequestLogRepository;

impl RequestLogRepository {
    pub async fn create(pool: &PgPool, data: CreateRequestLog) -> Result<RequestLog> {
        let log = sqlx::query_as::<_, RequestLog>(
            r#"
            INSERT INTO request_logs (
                server_id, tool_name, api_key_id, client_info,
                request_body, response_status, error_message, duration_ms
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, server_id, tool_name, api_key_id, client_info,
                      request_body, response_status, error_message, duration_ms, created_at
            "#,
        )
        .bind(data.server_id)
        .bind(&data.tool_name)
        .bind(data.api_key_id)
        .bind(&data.client_info)
        .bind(&data.request_body)
        .bind(&data.response_status)
        .bind(&data.error_message)
        .bind(data.duration_ms)
        .fetch_one(pool)
        .await?;

        Ok(log)
    }

    pub async fn list_by_server(
        pool: &PgPool,
        server_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<RequestLog>> {
        let logs = sqlx::query_as::<_, RequestLog>(
            r#"
            SELECT id, server_id, tool_name, api_key_id, client_info,
                   request_body, response_status, error_message, duration_ms, created_at
            FROM request_logs
            WHERE server_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(server_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        Ok(logs)
    }

    /// List request logs with filtering - optimized to use window function for count
    /// This executes a single query instead of two separate queries
    /// Uses sqlx::QueryBuilder for safe, efficient query construction
    ///
    /// # Scalability Note
    /// For large datasets (100K+ rows), consider using `list_by_server_cursor` instead
    /// as COUNT(*) OVER() can be expensive on large result sets.
    pub async fn list_by_server_filtered(
        pool: &PgPool,
        server_id: Uuid,
        limit: i64,
        offset: i64,
        status_filter: Option<&str>,
        time_range: Option<&str>,
        search: Option<&str>,
    ) -> Result<(Vec<RequestLog>, i64)> {
        use sqlx::QueryBuilder;

        let since = match time_range {
            Some("1h") => Some(Utc::now() - chrono::Duration::hours(1)),
            Some("24h") => Some(Utc::now() - chrono::Duration::hours(24)),
            Some("7d") => Some(Utc::now() - chrono::Duration::days(7)),
            Some("30d") => Some(Utc::now() - chrono::Duration::days(30)),
            _ => None,
        };

        // Build query using QueryBuilder for type-safe parameter binding
        let mut builder: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
            r#"SELECT id, server_id, tool_name, api_key_id, client_info,
                   request_body, response_status, error_message, duration_ms, created_at,
                   COUNT(*) OVER() as total_count
            FROM request_logs
            WHERE server_id = "#
        );

        builder.push_bind(server_id);

        // Add time filter
        if let Some(since_time) = since {
            builder.push(" AND created_at > ");
            builder.push_bind(since_time);
        }

        // Add status filter (using predefined safe conditions)
        match status_filter {
            Some("2xx") => {
                builder.push(" AND response_status = 'success'");
            }
            Some("4xx") => {
                builder.push(" AND (response_status LIKE 'client_%' OR response_status IN ('bad_request', 'unauthorized', 'forbidden', 'not_found'))");
            }
            Some("5xx") => {
                builder.push(" AND (response_status LIKE 'server_%' OR response_status IN ('error', 'internal_error'))");
            }
            _ => {}
        }

        // Add search filter
        if let Some(search_term) = search {
            let search_pattern = format!("%{}%", search_term);
            builder.push(" AND (tool_name ILIKE ");
            builder.push_bind(search_pattern.clone());
            builder.push(" OR error_message ILIKE ");
            builder.push_bind(search_pattern);
            builder.push(")");
        }

        builder.push(" ORDER BY created_at DESC LIMIT ");
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);

        let logs_with_count = builder
            .build_query_as::<RequestLogWithCount>()
            .fetch_all(pool)
            .await?;

        // Extract total count from first row (all rows have the same count)
        let total_count = logs_with_count.first().map(|r| r.total_count).unwrap_or(0);

        // Convert to RequestLog (without the count field)
        let logs: Vec<RequestLog> = logs_with_count
            .into_iter()
            .map(|r| RequestLog {
                id: r.id,
                server_id: r.server_id,
                tool_name: r.tool_name,
                api_key_id: r.api_key_id,
                client_info: r.client_info,
                request_body: r.request_body,
                response_status: r.response_status,
                error_message: r.error_message,
                duration_ms: r.duration_ms,
                created_at: r.created_at,
            })
            .collect();

        Ok((logs, total_count))
    }

    /// Scalability: Cursor-based pagination for large datasets
    /// Uses created_at + id as cursor to avoid expensive COUNT(*) OVER()
    /// Returns (logs, has_more) instead of total count
    pub async fn list_by_server_cursor(
        pool: &PgPool,
        server_id: Uuid,
        limit: i64,
        cursor: Option<(DateTime<Utc>, Uuid)>,  // (created_at, id) of last item
        status_filter: Option<&str>,
        time_range: Option<&str>,
        search: Option<&str>,
    ) -> Result<(Vec<RequestLog>, bool)> {
        use sqlx::QueryBuilder;

        let since = match time_range {
            Some("1h") => Some(Utc::now() - chrono::Duration::hours(1)),
            Some("24h") => Some(Utc::now() - chrono::Duration::hours(24)),
            Some("7d") => Some(Utc::now() - chrono::Duration::days(7)),
            Some("30d") => Some(Utc::now() - chrono::Duration::days(30)),
            _ => None,
        };

        // Build query using QueryBuilder for type-safe parameter binding
        let mut builder: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
            r#"SELECT id, server_id, tool_name, api_key_id, client_info,
                   request_body, response_status, error_message, duration_ms, created_at
            FROM request_logs
            WHERE server_id = "#
        );

        builder.push_bind(server_id);

        // Add cursor condition for keyset pagination
        if let Some((cursor_time, cursor_id)) = cursor {
            builder.push(" AND (created_at, id) < (");
            builder.push_bind(cursor_time);
            builder.push(", ");
            builder.push_bind(cursor_id);
            builder.push(")");
        }

        // Add time filter
        if let Some(since_time) = since {
            builder.push(" AND created_at > ");
            builder.push_bind(since_time);
        }

        // Add status filter (using predefined safe conditions)
        match status_filter {
            Some("2xx") => {
                builder.push(" AND response_status = 'success'");
            }
            Some("4xx") => {
                builder.push(" AND (response_status LIKE 'client_%' OR response_status IN ('bad_request', 'unauthorized', 'forbidden', 'not_found'))");
            }
            Some("5xx") => {
                builder.push(" AND (response_status LIKE 'server_%' OR response_status IN ('error', 'internal_error'))");
            }
            _ => {}
        }

        // Add search filter
        if let Some(search_term) = search {
            let search_pattern = format!("%{}%", search_term);
            builder.push(" AND (tool_name ILIKE ");
            builder.push_bind(search_pattern.clone());
            builder.push(" OR error_message ILIKE ");
            builder.push_bind(search_pattern);
            builder.push(")");
        }

        // Order by (created_at DESC, id DESC) for consistent cursor pagination
        builder.push(" ORDER BY created_at DESC, id DESC LIMIT ");
        // Fetch one extra to check if there are more results
        builder.push_bind(limit + 1);

        let mut logs = builder
            .build_query_as::<RequestLog>()
            .fetch_all(pool)
            .await?;

        // Check if there are more results
        let has_more = logs.len() > limit as usize;
        if has_more {
            logs.pop(); // Remove the extra item
        }

        Ok((logs, has_more))
    }

    pub async fn list_by_server_since(
        pool: &PgPool,
        server_id: Uuid,
        since: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<RequestLog>> {
        let logs = sqlx::query_as::<_, RequestLog>(
            r#"
            SELECT id, server_id, tool_name, api_key_id, client_info,
                   request_body, response_status, error_message, duration_ms, created_at
            FROM request_logs
            WHERE server_id = $1 AND created_at > $2
            ORDER BY created_at DESC
            LIMIT $3
            "#,
        )
        .bind(server_id)
        .bind(since)
        .bind(limit)
        .fetch_all(pool)
        .await?;

        Ok(logs)
    }

    pub async fn get_stats(
        pool: &PgPool,
        server_id: Uuid,
        since: DateTime<Utc>,
    ) -> Result<RequestLogStats> {
        let stats = sqlx::query_as::<_, RequestLogStats>(
            r#"
            SELECT
                COUNT(*)::BIGINT as total_requests,
                COUNT(*) FILTER (WHERE response_status = 'success')::BIGINT as success_count,
                COUNT(*) FILTER (WHERE response_status != 'success')::BIGINT as error_count,
                COALESCE(AVG(duration_ms)::FLOAT8, 0.0) as avg_duration_ms
            FROM request_logs
            WHERE server_id = $1 AND created_at > $2
            "#,
        )
        .bind(server_id)
        .bind(since)
        .fetch_one(pool)
        .await?;

        Ok(stats)
    }

    pub async fn get_tool_usage_stats(
        pool: &PgPool,
        server_id: Uuid,
        since: DateTime<Utc>,
    ) -> Result<Vec<ToolUsageStats>> {
        let stats = sqlx::query_as::<_, ToolUsageStats>(
            r#"
            SELECT
                COALESCE(tool_name, 'unknown') as tool_name,
                COUNT(*)::BIGINT as call_count,
                COUNT(*) FILTER (WHERE response_status != 'success')::BIGINT as error_count,
                COALESCE(AVG(duration_ms)::FLOAT8, 0.0) as avg_duration_ms
            FROM request_logs
            WHERE server_id = $1 AND created_at > $2
            GROUP BY tool_name
            ORDER BY call_count DESC
            "#,
        )
        .bind(server_id)
        .bind(since)
        .fetch_all(pool)
        .await?;

        Ok(stats)
    }

    pub async fn count_by_server(
        pool: &PgPool,
        server_id: Uuid,
        since: DateTime<Utc>,
    ) -> Result<i64> {
        let count: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM request_logs
            WHERE server_id = $1 AND created_at > $2
            "#,
        )
        .bind(server_id)
        .bind(since)
        .fetch_one(pool)
        .await?;

        Ok(count.0)
    }

    /// Delete old logs in batches to avoid long-running transactions
    /// Uses keyset pagination for better performance on large tables
    /// Returns total number of rows deleted
    ///
    /// # Scalability Note
    /// For very large tables (10M+ rows), consider implementing table partitioning:
    /// ```sql
    /// -- Create partitioned table (one-time migration)
    /// CREATE TABLE request_logs_partitioned (LIKE request_logs INCLUDING ALL)
    /// PARTITION BY RANGE (created_at);
    ///
    /// -- Create monthly partitions
    /// CREATE TABLE request_logs_2024_01 PARTITION OF request_logs_partitioned
    ///     FOR VALUES FROM ('2024-01-01') TO ('2024-02-01');
    ///
    /// -- Drop old partitions instead of deleting rows (instant operation)
    /// DROP TABLE request_logs_2023_01;
    /// ```
    pub async fn delete_old_logs(pool: &PgPool, before: DateTime<Utc>) -> Result<u64> {
        // Scalability: Configurable batch size via environment variable
        let batch_size: i64 = std::env::var("LOG_DELETE_BATCH_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(10000);

        // Scalability: Configurable max iterations
        let max_iterations: i32 = std::env::var("LOG_DELETE_MAX_ITERATIONS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1000);

        // Scalability: Configurable delay between batches (ms)
        let delay_ms: u64 = std::env::var("LOG_DELETE_DELAY_MS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(50);

        let mut total_deleted: u64 = 0;
        let mut iterations = 0;

        loop {
            iterations += 1;
            if iterations > max_iterations {
                tracing::warn!(
                    "delete_old_logs reached max iterations ({}), deleted {} rows so far",
                    max_iterations, total_deleted
                );
                break;
            }

            // Delete in batches using ctid for efficient scanning
            // This avoids the expensive subquery with LIMIT
            let result = sqlx::query(
                r#"
                DELETE FROM request_logs
                WHERE ctid IN (
                    SELECT ctid FROM request_logs
                    WHERE created_at < $1
                    LIMIT $2
                    FOR UPDATE SKIP LOCKED
                )
                "#
            )
            .bind(before)
            .bind(batch_size)
            .execute(pool)
            .await?;

            let deleted = result.rows_affected();
            total_deleted += deleted;

            // If we deleted less than batch_size, we're done
            if deleted < batch_size as u64 {
                break;
            }

            // Adaptive delay: configurable to balance cleanup speed vs database load
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
        }

        Ok(total_deleted)
    }

    /// Get batch stats for all servers in a workspace (single query)
    pub async fn get_batch_stats(
        pool: &PgPool,
        workspace_id: uuid::Uuid,
        since: DateTime<Utc>,
    ) -> Result<Vec<crate::models::ServerStatsSummary>> {
        let stats = sqlx::query_as::<_, crate::models::ServerStatsSummary>(
            r#"
            SELECT
                r.server_id,
                COUNT(*)::BIGINT as total_requests,
                COUNT(*) FILTER (WHERE r.response_status != 'success')::BIGINT as error_count
            FROM request_logs r
            JOIN mcp_servers s ON r.server_id = s.id
            WHERE s.workspace_id = $1 AND r.created_at > $2
            GROUP BY r.server_id
            "#,
        )
        .bind(workspace_id)
        .bind(since)
        .fetch_all(pool)
        .await?;

        Ok(stats)
    }
}
