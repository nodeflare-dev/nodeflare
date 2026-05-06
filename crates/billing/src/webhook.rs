use anyhow::{anyhow, Result};
use mcp_db::{DbPool, WorkspaceRepository};
use stripe::{Client, Event, EventObject, EventType, Subscription, SubscriptionId, Webhook};
use uuid::Uuid;

use crate::plans::{get_plan_by_price_id, Plan};

/// Stripe webhook handler
pub struct WebhookHandler {
    webhook_secret: String,
    db: DbPool,
    stripe_client: Client,
}

impl WebhookHandler {
    pub fn new(webhook_secret: &str, db: DbPool, stripe_api_key: &str) -> Self {
        Self {
            webhook_secret: webhook_secret.to_string(),
            db,
            stripe_client: Client::new(stripe_api_key),
        }
    }

    /// Verify and parse webhook event
    pub fn verify_event(&self, payload: &str, signature: &str) -> Result<Event> {
        Webhook::construct_event(payload, signature, &self.webhook_secret)
            .map_err(|e| anyhow!("Failed to verify webhook: {}", e))
    }

    /// Handle a Stripe webhook event
    pub async fn handle_event(&self, event: Event) -> Result<()> {
        tracing::info!("Processing Stripe webhook event: {:?}", event.type_);

        match event.type_ {
            EventType::CheckoutSessionCompleted => {
                self.handle_checkout_completed(event).await?;
            }
            EventType::CustomerSubscriptionCreated => {
                self.handle_subscription_created(event).await?;
            }
            EventType::CustomerSubscriptionUpdated => {
                self.handle_subscription_updated(event).await?;
            }
            EventType::CustomerSubscriptionDeleted => {
                self.handle_subscription_deleted(event).await?;
            }
            EventType::InvoicePaid => {
                self.handle_invoice_paid(event).await?;
            }
            EventType::InvoicePaymentFailed => {
                self.handle_invoice_payment_failed(event).await?;
            }
            _ => {
                tracing::debug!("Unhandled event type: {:?}", event.type_);
            }
        }

        Ok(())
    }

    async fn handle_checkout_completed(&self, event: Event) -> Result<()> {
        if let EventObject::CheckoutSession(session) = event.data.object {
            tracing::info!("Checkout completed for session: {}", session.id);

            // Get workspace ID from metadata
            let workspace_id = session
                .metadata
                .as_ref()
                .and_then(|m| m.get("workspace_id"))
                .and_then(|id| Uuid::parse_str(id).ok())
                .ok_or_else(|| anyhow!("Missing workspace_id in metadata"))?;

            // IDEMPOTENCY CHECK: Verify workspace doesn't already have this subscription
            // This prevents duplicate processing if webhook is sent multiple times
            let workspace = WorkspaceRepository::find_by_id(&self.db, workspace_id)
                .await?
                .ok_or_else(|| anyhow!("Workspace not found"))?;

            let subscription_id = session
                .subscription
                .as_ref()
                .map(|s| s.id().to_string())
                .ok_or_else(|| anyhow!("Missing subscription ID"))?;

            // If workspace already has this subscription ID, skip (idempotent)
            if workspace.stripe_subscription_id.as_ref() == Some(&subscription_id) {
                tracing::info!(
                    "Workspace {} already has subscription {}, skipping duplicate webhook",
                    workspace_id,
                    subscription_id
                );
                return Ok(());
            }

            // If workspace has a different subscription, this is suspicious
            if workspace.stripe_subscription_id.is_some() {
                tracing::warn!(
                    "Workspace {} already has a different subscription (existing: {:?}, new: {})",
                    workspace_id,
                    workspace.stripe_subscription_id,
                    subscription_id
                );
                // Still proceed - Stripe is the source of truth
            }

            // Get customer ID
            let customer_id = session
                .customer
                .as_ref()
                .map(|c| c.id().to_string())
                .ok_or_else(|| anyhow!("Missing customer ID"))?;

            // Update workspace with Stripe IDs
            WorkspaceRepository::update_stripe_ids(
                &self.db,
                workspace_id,
                &customer_id,
                &subscription_id,
            )
            .await?;

            // Get subscription to determine plan
            let subscription_id_parsed: SubscriptionId = subscription_id.parse()
                .map_err(|_| anyhow!("Invalid subscription ID"))?;
            let subscription = Subscription::retrieve(&self.stripe_client, &subscription_id_parsed, &[])
                .await
                .map_err(|e| anyhow!("Failed to retrieve subscription: {}", e))?;

            // Get plan from price
            let plan = subscription
                .items
                .data
                .first()
                .and_then(|item| item.price.as_ref())
                .and_then(|price| get_plan_by_price_id(&price.id))
                .unwrap_or(Plan::Free);

            // Update workspace plan
            WorkspaceRepository::update_plan(&self.db, workspace_id, plan.into())
                .await?;

            // Update subscription status
            let status = subscription.status.to_string();
            let period_end = chrono::DateTime::from_timestamp(subscription.current_period_end, 0);

            WorkspaceRepository::update_subscription_status(
                &self.db,
                workspace_id,
                &status,
                period_end,
            ).await?;

            tracing::info!(
                "Updated workspace {} with plan {:?}, customer {}, subscription {}",
                workspace_id,
                plan,
                customer_id,
                subscription_id
            );
        }

        Ok(())
    }

    async fn handle_subscription_created(&self, event: Event) -> Result<()> {
        if let EventObject::Subscription(subscription) = event.data.object {
            tracing::info!("Subscription created: {}", subscription.id);

            // Get workspace ID from metadata
            let workspace_id = subscription
                .metadata
                .get("workspace_id")
                .and_then(|id| Uuid::parse_str(id).ok())
                .ok_or_else(|| anyhow!("Missing workspace_id in subscription metadata"))?;

            // Get plan from price
            let plan = subscription
                .items
                .data
                .first()
                .and_then(|item| item.price.as_ref())
                .and_then(|price| get_plan_by_price_id(&price.id))
                .unwrap_or(Plan::Free);

            // Update workspace plan
            WorkspaceRepository::update_plan(&self.db, workspace_id, plan.into())
                .await?;

            // Set subscription status and period end
            let status = subscription.status.to_string();
            let period_end = chrono::DateTime::from_timestamp(subscription.current_period_end, 0);

            WorkspaceRepository::update_subscription_status(
                &self.db,
                workspace_id,
                &status,
                period_end,
            ).await?;

            tracing::info!(
                "Created subscription for workspace {} with plan {:?}, status {}, period ends: {:?}",
                workspace_id, plan, status, period_end
            );
        }

        Ok(())
    }

    async fn handle_subscription_updated(&self, event: Event) -> Result<()> {
        if let EventObject::Subscription(subscription) = event.data.object {
            tracing::info!("Subscription updated: {}", subscription.id);

            // Get workspace ID from metadata
            let workspace_id = subscription
                .metadata
                .get("workspace_id")
                .and_then(|id| Uuid::parse_str(id).ok())
                .ok_or_else(|| anyhow!("Missing workspace_id in subscription metadata"))?;

            // Get plan from price
            let plan = subscription
                .items
                .data
                .first()
                .and_then(|item| item.price.as_ref())
                .and_then(|price| get_plan_by_price_id(&price.id))
                .unwrap_or(Plan::Free);

            // Update workspace plan
            WorkspaceRepository::update_plan(&self.db, workspace_id, plan.into())
                .await?;

            // Update subscription status and period end
            let status = subscription.status.to_string();
            let period_end = chrono::DateTime::from_timestamp(subscription.current_period_end, 0);

            WorkspaceRepository::update_subscription_status(
                &self.db,
                workspace_id,
                &status,
                period_end,
            ).await?;

            tracing::info!(
                "Updated workspace {} to plan {:?} with status {} (period ends: {:?})",
                workspace_id, plan, status, period_end
            );
        }

        Ok(())
    }

    async fn handle_subscription_deleted(&self, event: Event) -> Result<()> {
        if let EventObject::Subscription(subscription) = event.data.object {
            tracing::info!("Subscription deleted: {}", subscription.id);

            // Try to get workspace ID from metadata first
            let workspace_id = if let Some(id) = subscription
                .metadata
                .get("workspace_id")
                .and_then(|id| Uuid::parse_str(id).ok())
            {
                Some(id)
            } else {
                // Fallback: find workspace by subscription ID
                let sub_id = subscription.id.as_str();
                if let Ok(Some(ws)) = WorkspaceRepository::find_by_stripe_subscription(&self.db, sub_id).await {
                    Some(ws.id)
                } else {
                    // Fallback: find workspace by customer ID
                    let customer_id = subscription.customer.id().to_string();
                    if let Ok(Some(ws)) = WorkspaceRepository::find_by_stripe_customer(&self.db, &customer_id).await {
                        // Only match if this subscription ID matches
                        if ws.stripe_subscription_id.as_deref() == Some(sub_id) {
                            Some(ws.id)
                        } else {
                            tracing::warn!(
                                "Customer {} has different subscription (db: {:?}, event: {}), skipping",
                                customer_id, ws.stripe_subscription_id, sub_id
                            );
                            None
                        }
                    } else {
                        None
                    }
                }
            };

            let Some(workspace_id) = workspace_id else {
                tracing::warn!(
                    "Could not find workspace for deleted subscription {}, skipping",
                    subscription.id
                );
                return Ok(());
            };

            // Downgrade to free plan
            WorkspaceRepository::update_plan(&self.db, workspace_id, mcp_common::types::Plan::Free)
                .await?;

            // Clear Stripe subscription ID
            WorkspaceRepository::clear_stripe_subscription(&self.db, workspace_id)
                .await?;

            tracing::info!("Downgraded workspace {} to Free plan", workspace_id);
        }

        Ok(())
    }

    async fn handle_invoice_payment_failed(&self, event: Event) -> Result<()> {
        if let EventObject::Invoice(invoice) = event.data.object {
            tracing::warn!(
                "Invoice payment failed: {}",
                invoice.id.as_str()
            );

            // Get customer ID from invoice
            if let Some(customer) = invoice.customer {
                let customer_id = customer.id().to_string();

                // Find workspace by customer ID
                if let Ok(Some(workspace)) = WorkspaceRepository::find_by_stripe_customer(&self.db, &customer_id).await {
                    // Get attempt count from invoice
                    let attempt_count = invoice.attempt_count.unwrap_or(0);

                    // Update subscription status based on attempt count
                    let new_status = if attempt_count >= 3 {
                        // After 3 failed attempts, mark as unpaid (service will be blocked)
                        tracing::warn!(
                            "Workspace {} marked as unpaid after {} failed payment attempts",
                            workspace.id,
                            attempt_count
                        );
                        "unpaid"
                    } else {
                        // Mark as past_due (grace period, service continues but user warned)
                        tracing::warn!(
                            "Workspace {} marked as past_due (attempt {})",
                            workspace.id,
                            attempt_count
                        );
                        "past_due"
                    };

                    if let Err(e) = WorkspaceRepository::update_subscription_status(
                        &self.db,
                        workspace.id,
                        new_status,
                        None,
                    ).await {
                        tracing::error!("Failed to update subscription status: {}", e);
                    }
                }
            }
        }

        Ok(())
    }

    async fn handle_invoice_paid(&self, event: Event) -> Result<()> {
        if let EventObject::Invoice(invoice) = event.data.object {
            tracing::info!(
                "Invoice paid: {} for amount {}",
                invoice.id.as_str(),
                invoice.amount_paid.unwrap_or(0)
            );

            // On successful payment, restore active status
            if let Some(customer) = invoice.customer {
                let customer_id = customer.id().to_string();

                if let Ok(Some(workspace)) = WorkspaceRepository::find_by_stripe_customer(&self.db, &customer_id).await {
                    // Only update if status was past_due or unpaid
                    if let Some(ref status) = workspace.subscription_status {
                        if status == "past_due" || status == "unpaid" {
                            tracing::info!(
                                "Restoring workspace {} to active status after payment",
                                workspace.id
                            );

                            // Get period end from subscription
                            let period_end = invoice.lines.as_ref()
                                .and_then(|lines| lines.data.first())
                                .and_then(|line| line.period.as_ref())
                                .and_then(|period| period.end)
                                .and_then(|end| chrono::DateTime::from_timestamp(end, 0));

                            if let Err(e) = WorkspaceRepository::update_subscription_status(
                                &self.db,
                                workspace.id,
                                "active",
                                period_end,
                            ).await {
                                tracing::error!("Failed to restore subscription status: {}", e);
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

impl From<Plan> for mcp_common::types::Plan {
    fn from(plan: Plan) -> Self {
        match plan {
            Plan::Free => mcp_common::types::Plan::Free,
            Plan::Pro => mcp_common::types::Plan::Pro,
            Plan::Team => mcp_common::types::Plan::Team,
            Plan::Enterprise => mcp_common::types::Plan::Enterprise,
        }
    }
}
