use anyhow::{anyhow, Result};
use stripe::{
    CheckoutSession, CheckoutSessionMode, Client, CreateCheckoutSession,
    CreateCheckoutSessionLineItems, CreateCustomer, CreateBillingPortalSession,
    Customer, CustomerId, Subscription, SubscriptionId,
    Invoice, ListInvoices,
    SubscriptionItemId, PriceId,
};
use uuid::Uuid;

use crate::plans::{get_plan_by_price_id, Plan};

/// Stripe billing service
#[derive(Clone)]
pub struct BillingService {
    client: Client,
    success_url: String,
    cancel_url: String,
    portal_return_url: String,
}

impl BillingService {
    pub fn new(api_key: &str, base_url: &str) -> Self {
        let client = Client::new(api_key);

        Self {
            client,
            success_url: format!("{}/dashboard/billing/success", base_url),
            cancel_url: format!("{}/dashboard/billing/cancel", base_url),
            portal_return_url: format!("{}/dashboard/billing", base_url),
        }
    }

    /// Get the Stripe client for advanced operations
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Create a new Stripe customer
    pub async fn create_customer(&self, email: &str, name: &str, user_id: Uuid) -> Result<Customer> {
        let mut params = CreateCustomer::new();
        params.email = Some(email);
        params.name = Some(name);
        params.metadata = Some(
            [("user_id".to_string(), user_id.to_string())]
                .into_iter()
                .collect(),
        );

        Customer::create(&self.client, params)
            .await
            .map_err(|e| anyhow!("Failed to create Stripe customer: {}", e))
    }

    /// Create a checkout session for subscription
    pub async fn create_checkout_session(
        &self,
        customer_id: &str,
        price_id: &str,
        workspace_id: Uuid,
    ) -> Result<CheckoutSession> {
        let customer_id: CustomerId = customer_id.parse().map_err(|_| anyhow!("Invalid customer ID"))?;
        let mut params = CreateCheckoutSession::new();
        params.customer = Some(customer_id);
        params.mode = Some(CheckoutSessionMode::Subscription);
        params.success_url = Some(&self.success_url);
        params.cancel_url = Some(&self.cancel_url);
        params.line_items = Some(vec![CreateCheckoutSessionLineItems {
            price: Some(price_id.to_string()),
            quantity: Some(1),
            ..Default::default()
        }]);
        params.metadata = Some(
            [("workspace_id".to_string(), workspace_id.to_string())]
                .into_iter()
                .collect(),
        );
        params.subscription_data = Some(stripe::CreateCheckoutSessionSubscriptionData {
            metadata: Some(
                [("workspace_id".to_string(), workspace_id.to_string())]
                    .into_iter()
                    .collect(),
            ),
            ..Default::default()
        });

        CheckoutSession::create(&self.client, params)
            .await
            .map_err(|e| anyhow!("Failed to create checkout session: {}", e))
    }

    /// Create a customer portal session for managing subscription
    pub async fn create_portal_session(&self, customer_id: &str) -> Result<stripe::BillingPortalSession> {
        let customer_id: CustomerId = customer_id.parse().map_err(|_| anyhow!("Invalid customer ID"))?;
        let mut params = CreateBillingPortalSession::new(customer_id);
        params.return_url = Some(&self.portal_return_url);

        stripe::BillingPortalSession::create(&self.client, params)
            .await
            .map_err(|e| anyhow!("Failed to create portal session: {}", e))
    }

    /// Get subscription details
    pub async fn get_subscription(&self, subscription_id: &str) -> Result<Subscription> {
        let id: SubscriptionId = subscription_id.parse().map_err(|_| anyhow!("Invalid subscription ID"))?;
        Subscription::retrieve(&self.client, &id, &[])
            .await
            .map_err(|e| anyhow!("Failed to get subscription: {}", e))
    }

    /// Get subscription period end as DateTime<Utc>
    pub async fn get_subscription_period_end(&self, subscription_id: &str) -> Result<chrono::DateTime<chrono::Utc>> {
        let subscription = self.get_subscription(subscription_id).await?;

        // current_period_end is a Unix timestamp
        let period_end_timestamp = subscription.current_period_end;

        chrono::DateTime::from_timestamp(period_end_timestamp, 0)
            .ok_or_else(|| anyhow!("Invalid period end timestamp: {}", period_end_timestamp))
    }

    /// List all subscriptions for a customer (including canceled)
    pub async fn list_subscriptions(&self, customer_id: &str) -> Result<Vec<Subscription>> {
        let customer_id: CustomerId = customer_id.parse().map_err(|_| anyhow!("Invalid customer ID"))?;

        let mut params = stripe::ListSubscriptions::new();
        params.customer = Some(customer_id);
        // Include all statuses by not filtering

        let subscriptions = Subscription::list(&self.client, &params)
            .await
            .map_err(|e| anyhow!("Failed to list subscriptions: {}", e))?;

        Ok(subscriptions.data)
    }

    /// Cancel a subscription at period end (user keeps access until billing period ends)
    pub async fn cancel_subscription(&self, subscription_id: &str) -> Result<Subscription> {
        let id: SubscriptionId = subscription_id.parse().map_err(|_| anyhow!("Invalid subscription ID"))?;

        // Set cancel_at_period_end instead of immediate cancellation
        let mut params = stripe::UpdateSubscription::new();
        params.cancel_at_period_end = Some(true);

        Subscription::update(&self.client, &id, params)
            .await
            .map_err(|e| anyhow!("Failed to cancel subscription: {}", e))
    }

    /// Update subscription to a different plan (upgrade/downgrade)
    /// This immediately changes the plan and prorates the billing
    pub async fn update_subscription_plan(
        &self,
        subscription_id: &str,
        new_price_id: &str,
    ) -> Result<Subscription> {
        let sub_id: SubscriptionId = subscription_id.parse()
            .map_err(|_| anyhow!("Invalid subscription ID"))?;

        // First, get the current subscription to find the plan item
        let subscription = self.get_subscription(subscription_id).await?;

        // Find the plan subscription item
        let plan_item = subscription.items.data.first()
            .ok_or_else(|| anyhow!("No plan subscription item found"))?;

        let item_id: SubscriptionItemId = plan_item.id.clone();
        let price_id: PriceId = new_price_id.parse()
            .map_err(|_| anyhow!("Invalid price ID"))?;

        // Update the subscription item with the new price
        // Using items array to update the subscription
        let mut params = stripe::UpdateSubscription::new();
        params.items = Some(vec![stripe::UpdateSubscriptionItems {
            id: Some(item_id.to_string()),
            price: Some(price_id.to_string()),
            ..Default::default()
        }]);
        // Prorate: charge/credit the difference immediately
        // Note: Using the correct enum variant for proration
        params.proration_behavior = Some(stripe::generated::billing::subscription::SubscriptionProrationBehavior::CreateProrations);
        // If it was set to cancel, clear that
        params.cancel_at_period_end = Some(false);

        Subscription::update(&self.client, &sub_id, params)
            .await
            .map_err(|e| anyhow!("Failed to update subscription plan: {}", e))
    }

    /// Immediately cancel a subscription (use sparingly - user loses access immediately)
    pub async fn cancel_subscription_immediately(&self, subscription_id: &str) -> Result<Subscription> {
        let id: SubscriptionId = subscription_id.parse().map_err(|_| anyhow!("Invalid subscription ID"))?;
        Subscription::cancel(&self.client, &id, stripe::CancelSubscription::default())
            .await
            .map_err(|e| anyhow!("Failed to cancel subscription: {}", e))
    }

    /// Get customer by ID
    pub async fn get_customer(&self, customer_id: &str) -> Result<Customer> {
        let id: CustomerId = customer_id.parse().map_err(|_| anyhow!("Invalid customer ID"))?;
        Customer::retrieve(&self.client, &id, &[])
            .await
            .map_err(|e| anyhow!("Failed to get customer: {}", e))
    }

    /// Get default payment method for a customer
    pub async fn get_default_payment_method(&self, customer_id: &str) -> Result<Option<PaymentMethodDetails>> {
        let id: CustomerId = customer_id.parse().map_err(|_| anyhow!("Invalid customer ID"))?;

        // List payment methods for the customer
        let mut params = stripe::ListPaymentMethods::new();
        params.customer = Some(id);
        params.type_ = Some(stripe::PaymentMethodTypeFilter::Card);
        params.limit = Some(1);

        let payment_methods = stripe::PaymentMethod::list(&self.client, &params)
            .await
            .map_err(|e| anyhow!("Failed to list payment methods: {}", e))?;

        if let Some(pm) = payment_methods.data.first() {
            if let Some(card) = &pm.card {
                // card.brand and card.last4 are Strings
                let brand = if card.brand.is_empty() {
                    "card".to_string()
                } else {
                    card.brand.to_lowercase()
                };

                return Ok(Some(PaymentMethodDetails {
                    brand,
                    last4: card.last4.clone(),
                    exp_month: card.exp_month as u32,
                    exp_year: card.exp_year as u32,
                }));
            }
        }

        Ok(None)
    }

    /// Get plan from subscription
    pub fn get_plan_from_subscription(&self, subscription: &Subscription) -> Plan {
        subscription
            .items
            .data
            .first()
            .and_then(|item| item.price.as_ref())
            .and_then(|price| price.id.as_str().parse().ok())
            .and_then(|price_id: String| get_plan_by_price_id(&price_id))
            .unwrap_or(Plan::Free)
    }

    /// List invoices for a customer
    pub async fn list_invoices(&self, customer_id: &str, limit: i64) -> Result<Vec<Invoice>> {
        let customer_id: CustomerId = customer_id.parse().map_err(|_| anyhow!("Invalid customer ID"))?;
        let mut params = ListInvoices::new();
        params.customer = Some(customer_id);
        params.limit = Some(limit as u64);

        Invoice::list(&self.client, &params)
            .await
            .map(|list| list.data)
            .map_err(|e| anyhow!("Failed to list invoices: {}", e))
    }
}

/// Payment method details
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PaymentMethodDetails {
    pub brand: String,
    pub last4: String,
    pub exp_month: u32,
    pub exp_year: u32,
}

/// Subscription status response
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SubscriptionStatus {
    pub plan: Plan,
    pub status: String,
    pub current_period_end: Option<i64>,
    pub cancel_at_period_end: bool,
    pub stripe_subscription_id: Option<String>,
    pub stripe_customer_id: Option<String>,
}

impl Default for SubscriptionStatus {
    fn default() -> Self {
        Self {
            plan: Plan::Free,
            status: "active".to_string(),
            current_period_end: None,
            cancel_at_period_end: false,
            stripe_subscription_id: None,
            stripe_customer_id: None,
        }
    }
}
