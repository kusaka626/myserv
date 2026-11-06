use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: Uuid,
    pub server_binding: String,
    pub receipt_email: String,
}
#[derive(Serialize, Deserialize)]
pub struct Payment {
    pub status: String,
    pub confirmation_url: Option<String>,
    pub license: Option<String>,
}

pub struct Billing {
    url: url::Url,
    http: reqwest::Client,
}
impl Billing {
    pub fn new(url: &str) -> Result<Self> {
        let url = url::Url::parse(url)?;
        ensure!(
            url.scheme() == "https" && url.username().is_empty() && url.password().is_none(),
            "Billing API должен использовать HTTPS"
        );
        Ok(Self {
            url,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()?,
        })
    }
    pub async fn create(&self, order: &Order, access: &str) -> Result<Payment> {
        Ok(self
            .http
            .post(self.url.join("/v1/payments")?)
            .bearer_auth(access)
            .json(order)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }
    pub async fn check(&self, order: Uuid, access: &str) -> Result<Payment> {
        Ok(self
            .http
            .get(self.url.join(&format!("/v1/payments/{order}"))?)
            .bearer_auth(access)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }
}
