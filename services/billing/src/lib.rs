use anyhow::{Result, ensure};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::SigningKey;
use myserv_core::{
    billing::{Order, Payment},
    license::{Grant, sign},
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use uuid::Uuid;

pub struct Service {
    db: Mutex<Connection>,
    http: reqwest::Client,
    shop: String,
    secret: String,
    signing: SigningKey,
    return_url: String,
    vat_code: u8,
    allow_test: bool,
    provider_base: String,
}

impl Service {
    pub fn from_env() -> Result<Self> {
        let read = |name| std::env::var(name).map_err(|_| anyhow::anyhow!("Missing {name}"));
        let bytes: [u8; 32] = URL_SAFE_NO_PAD
            .decode(read("MYSERV_SIGNING_KEY")?)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("Signing key must be 32 bytes, base64url"))?;
        let db = Connection::open(read("MYSERV_DATABASE")?)?;
        db.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE IF NOT EXISTS orders (id TEXT PRIMARY KEY, binding TEXT NOT NULL, access_hash TEXT NOT NULL, provider_id TEXT, created_at INTEGER NOT NULL DEFAULT (unixepoch()));")?;
        let return_url = read("MYSERV_RETURN_URL")?;
        ensure!(
            return_url.starts_with("https://"),
            "Return URL must use HTTPS"
        );
        let vat_code = read("MYSERV_VAT_CODE")?.parse()?;
        ensure!(
            (1..=12).contains(&vat_code),
            "Invalid receipt VAT code; choose according to your taxation"
        );
        Ok(Self {
            db: Mutex::new(db),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(20))
                .build()?,
            shop: read("YOOKASSA_SHOP_ID")?,
            secret: read("YOOKASSA_SECRET_KEY")?,
            signing: SigningKey::from_bytes(&bytes),
            return_url,
            vat_code,
            allow_test: std::env::var("MYSERV_ALLOW_TEST_PAYMENTS").as_deref() == Ok("true"),
            provider_base: "https://api.yookassa.ru/v3".into(),
        })
    }
    async fn provider(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
        id: Option<Uuid>,
    ) -> Result<ProviderPayment> {
        let mut req = self
            .http
            .request(method, format!("{}/{path}", self.provider_base))
            .basic_auth(&self.shop, Some(&self.secret));
        if let Some(id) = id {
            req = req.header("Idempotence-Key", id.to_string());
        }
        if let Some(body) = body {
            req = req.json(&body);
        }
        Ok(req.send().await?.error_for_status()?.json().await?)
    }
}

#[derive(Deserialize)]
pub struct ProviderPayment {
    pub id: String,
    pub status: String,
    #[serde(default)]
    pub paid: bool,
    #[serde(default)]
    pub test: bool,
    pub amount: Value,
    #[serde(default)]
    pub metadata: Value,
    #[serde(default)]
    pub confirmation: Value,
}

pub fn verified_grant(
    p: &ProviderPayment,
    id: Uuid,
    binding: &str,
    allow_test: bool,
) -> Result<Option<Grant>> {
    ensure!(
        p.metadata["order_id"] == id.to_string() && p.metadata["server_binding"] == binding,
        "Payment metadata mismatch"
    );
    ensure!(
        p.amount["value"] == "199.00" && p.amount["currency"] == "RUB",
        "Payment amount mismatch"
    );
    ensure!(!p.test || allow_test, "Test payments are disabled");
    if p.status != "succeeded" {
        return Ok(None);
    }
    ensure!(p.paid, "Payment is not paid");
    Ok(Some(Grant {
        product: myserv_core::PRODUCT.into(),
        order_id: id.to_string(),
        server_binding: binding.into(),
        amount_kopeks: myserv_core::PRICE_KOPEKS,
        currency: "RUB".into(),
    }))
}

struct ApiError(StatusCode, &'static str);
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error":self.1}))).into_response()
    }
}
fn internal(_: impl std::fmt::Display) -> ApiError {
    ApiError(
        StatusCode::BAD_GATEWAY,
        "Платёжный сервис временно недоступен",
    )
}
fn access(headers: &HeaderMap) -> Result<String, ApiError> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(ApiError(StatusCode::UNAUTHORIZED, "Требуется ключ заказа"))?;
    if token.len() != 64 || !token.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(ApiError(StatusCode::UNAUTHORIZED, "Неверный ключ заказа"));
    }
    Ok(format!("{:x}", Sha256::digest(token)))
}

pub fn router(service: Arc<Service>) -> Router {
    Router::new()
        .route("/health", get(|| async { Json(json!({"status":"ok"})) }))
        .route("/v1/payments", post(create))
        .route("/v1/payments/{id}", get(check))
        .layer(axum::extract::DefaultBodyLimit::max(8192))
        .with_state(service)
}

async fn create(
    State(s): State<Arc<Service>>,
    headers: HeaderMap,
    Json(order): Json<Order>,
) -> Result<Json<Payment>, ApiError> {
    let hash = access(&headers)?;
    if order.server_binding.len() != 64
        || !order.server_binding.bytes().all(|c| c.is_ascii_hexdigit())
        || order.id.get_version_num() != 4
        || order.receipt_email.len() > 254
        || !order.receipt_email.contains('@')
        || order.receipt_email.bytes().any(|b| b.is_ascii_whitespace())
    {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Неверные данные заказа или email чека",
        ));
    }
    let existing = {
        let db = s.db.lock().map_err(internal)?;
        db.execute(
            "INSERT OR IGNORE INTO orders (id,binding,access_hash) VALUES (?1,?2,?3)",
            params![order.id.to_string(), order.server_binding, hash],
        )
        .map_err(internal)?;
        let (binding, stored, pid, age): (String, String, Option<String>, i64) = db
            .query_row(
                "SELECT binding,access_hash,provider_id,unixepoch()-created_at FROM orders WHERE id=?1",
                [order.id.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .map_err(internal)?;
        if binding != order.server_binding || stored != hash {
            return Err(ApiError(StatusCode::CONFLICT, "Заказ уже существует"));
        }
        if pid.is_none() && age > 23 * 3600 {
            return Err(ApiError(
                StatusCode::CONFLICT,
                "Срок безопасного повтора истёк. Оператор должен сверить заказ в ЮKassa перед новой оплатой",
            ));
        }
        pid
    };
    if let Some(id) = existing {
        return response(&s, &id, order.id, &order.server_binding).await;
    }
    let body = json!({
        "amount":{"value":"199.00","currency":"RUB"},"capture":true,
        "confirmation":{"type":"redirect","return_url":s.return_url},
        "description":"MyServ: обучение и автоматическая настройка собственного VPS",
        "metadata":{"order_id":order.id.to_string(),"server_binding":order.server_binding},
        "receipt":{"customer":{"email":order.receipt_email},"items":[{"description":"Автоматическая настройка собственного сервера","quantity":"1.00","amount":{"value":"199.00","currency":"RUB"},"vat_code":s.vat_code,"payment_subject":"service","payment_mode":"full_payment"}]}
    });
    let p = s
        .provider(
            reqwest::Method::POST,
            "payments",
            Some(body),
            Some(order.id),
        )
        .await
        .map_err(internal)?;
    // Persist provider ID before returning; retries use the same provider idempotency key.
    s.db.lock()
        .map_err(internal)?
        .execute(
            "UPDATE orders SET provider_id=?1 WHERE id=?2",
            params![p.id, order.id.to_string()],
        )
        .map_err(internal)?;
    payment_response(&s, p, order.id, &order.server_binding)
}

async fn check(
    State(s): State<Arc<Service>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Payment>, ApiError> {
    let hash = access(&headers)?;
    let stored: Option<(String, Option<String>)> =
        s.db.lock()
            .map_err(internal)?
            .query_row(
                "SELECT binding,provider_id FROM orders WHERE id=?1 AND access_hash=?2",
                params![id.to_string(), hash],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(internal)?;
    let (binding, pid) = stored.ok_or(ApiError(StatusCode::NOT_FOUND, "Заказ не найден"))?;
    let pid = pid.ok_or(ApiError(
        StatusCode::CONFLICT,
        "Повторите создание заказа с прежним ключом",
    ))?;
    response(&s, &pid, id, &binding).await
}
async fn response(
    s: &Service,
    pid: &str,
    id: Uuid,
    binding: &str,
) -> Result<Json<Payment>, ApiError> {
    let p = s
        .provider(reqwest::Method::GET, &format!("payments/{pid}"), None, None)
        .await
        .map_err(internal)?;
    if p.id != pid {
        return Err(internal("Provider ID mismatch"));
    }
    payment_response(s, p, id, binding)
}
fn payment_response(
    s: &Service,
    p: ProviderPayment,
    id: Uuid,
    binding: &str,
) -> Result<Json<Payment>, ApiError> {
    let grant = verified_grant(&p, id, binding, s.allow_test).map_err(internal)?;
    let license = grant
        .map(|g| sign(&g, &s.signing))
        .transpose()
        .map_err(internal)?;
    let confirmation_url = p.confirmation["confirmation_url"]
        .as_str()
        .map(String::from);
    Ok(Json(Payment {
        status: p.status,
        confirmation_url,
        license,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn paid(id: Uuid) -> ProviderPayment {
        serde_json::from_value(json!({"id":"provider-id","status":"succeeded","paid":true,"test":false,"amount":{"value":"199.00","currency":"RUB"},"metadata":{"order_id":id.to_string(),"server_binding":"server"}})).unwrap()
    }
    #[test]
    fn only_confirmed_exact_amount_unlocks() {
        let id = Uuid::new_v4();
        let mut p = paid(id);
        assert!(verified_grant(&p, id, "server", false).unwrap().is_some());
        p.amount["value"] = json!("1.00");
        assert!(verified_grant(&p, id, "server", false).is_err());
        p = paid(id);
        p.paid = false;
        assert!(verified_grant(&p, id, "server", false).is_err());
        p = paid(id);
        p.test = true;
        assert!(verified_grant(&p, id, "server", false).is_err());
        p = paid(id);
        p.status = "pending".into();
        assert!(verified_grant(&p, id, "server", false).unwrap().is_none());
        assert!(verified_grant(&paid(id), id, "other", false).is_err());
        assert!(verified_grant(&paid(id), Uuid::new_v4(), "server", false).is_err());
    }
}

#[cfg(test)]
mod api_tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tower::ServiceExt;

    #[derive(Clone)]
    struct Provider {
        response: Arc<Mutex<Value>>,
        creates: Arc<AtomicUsize>,
    }
    async fn create_fixture(
        State(p): State<Provider>,
        headers: HeaderMap,
        Json(body): Json<Value>,
    ) -> Json<Value> {
        assert_eq!(body["amount"], json!({"value":"199.00","currency":"RUB"}));
        assert_eq!(body["capture"], true);
        assert_eq!(
            headers["idempotence-key"].to_str().unwrap(),
            body["metadata"]["order_id"].as_str().unwrap()
        );
        p.creates.fetch_add(1, Ordering::SeqCst);
        let mut response = p.response.lock().unwrap();
        response["metadata"] = body["metadata"].clone();
        Json(response.clone())
    }
    async fn fixture_status(State(p): State<Provider>) -> Json<Value> {
        Json(p.response.lock().unwrap().clone())
    }
    async fn request(
        app: Router,
        method: &str,
        path: &str,
        token: Option<&str>,
        body: Value,
    ) -> (StatusCode, Value) {
        let mut req = Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json");
        if let Some(token) = token {
            req = req.header("authorization", format!("Bearer {token}"));
        }
        let result = app
            .oneshot(req.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap();
        let status = result.status();
        let bytes = result.into_body().collect().await.unwrap().to_bytes();
        (status, serde_json::from_slice(&bytes).unwrap())
    }
    #[tokio::test]
    async fn payment_lifecycle_idempotency_and_order_access() {
        let provider = Provider {
            response: Arc::new(Mutex::new(
                json!({"id":"fixture","status":"pending","paid":false,"test":false,"amount":{"value":"199.00","currency":"RUB"},"confirmation":{"confirmation_url":"https://yookassa.ru/fixture"}}),
            )),
            creates: Arc::new(AtomicUsize::new(0)),
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let fake = Router::new()
            .route("/payments", post(create_fixture))
            .route("/payments/fixture", get(fixture_status))
            .with_state(provider.clone());
        let task = tokio::spawn(async move { axum::serve(listener, fake).await.unwrap() });
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE orders (id TEXT PRIMARY KEY,binding TEXT NOT NULL,access_hash TEXT NOT NULL,provider_id TEXT,created_at INTEGER NOT NULL DEFAULT (unixepoch()));").unwrap();
        let key = SigningKey::generate(&mut rand::rngs::OsRng);
        let public = URL_SAFE_NO_PAD.encode(key.verifying_key().to_bytes());
        let service = Arc::new(Service {
            db: Mutex::new(db),
            http: reqwest::Client::builder().no_proxy().build().unwrap(),
            shop: "fixture".into(),
            secret: "fixture".into(),
            signing: key,
            return_url: "https://example.com/return".into(),
            vat_code: 1,
            allow_test: false,
            provider_base: format!("http://{address}"),
        });
        let app = router(service.clone());
        let token = "a".repeat(64);
        let binding = "b".repeat(64);
        let id = Uuid::new_v4();
        let body = json!({"id":id,"server_binding":binding,"receipt_email":"test@example.com"});
        let (code, p) = request(
            app.clone(),
            "POST",
            "/v1/payments",
            Some(&token),
            body.clone(),
        )
        .await;
        assert_eq!(code, StatusCode::OK);
        assert!(p["license"].is_null());
        let (code, _) = request(
            app.clone(),
            "POST",
            "/v1/payments",
            Some(&token),
            body.clone(),
        )
        .await;
        assert_eq!(code, StatusCode::OK);
        assert_eq!(provider.creates.load(Ordering::SeqCst), 1);
        let path = format!("/v1/payments/{id}");
        assert_eq!(
            request(app.clone(), "GET", &path, None, Value::Null)
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            request(
                app.clone(),
                "GET",
                &path,
                Some(&"c".repeat(64)),
                Value::Null
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            request(
                app.clone(),
                "POST",
                "/v1/payments",
                Some(&"c".repeat(64)),
                body.clone()
            )
            .await
            .0,
            StatusCode::CONFLICT
        );
        {
            let mut r = provider.response.lock().unwrap();
            r["status"] = json!("succeeded");
            r["paid"] = json!(true);
        }
        let (code, p) = request(app.clone(), "GET", &path, Some(&token), Value::Null).await;
        assert_eq!(code, StatusCode::OK);
        assert!(
            myserv_core::license::verify(p["license"].as_str().unwrap(), &public, &binding).is_ok()
        );
        provider.response.lock().unwrap()["amount"]["value"] = json!("1.00");
        assert_eq!(
            request(app.clone(), "GET", &path, Some(&token), Value::Null)
                .await
                .0,
            StatusCode::BAD_GATEWAY
        );
        // Unknown create outcome must not be retried after the provider's idempotency window.
        service
            .db
            .lock()
            .unwrap()
            .execute(
                "UPDATE orders SET provider_id=NULL,created_at=unixepoch()-86401 WHERE id=?1",
                [id.to_string()],
            )
            .unwrap();
        assert_eq!(
            request(app, "POST", "/v1/payments", Some(&token), body)
                .await
                .0,
            StatusCode::CONFLICT
        );
        assert_eq!(provider.creates.load(Ordering::SeqCst), 1);
        task.abort();
    }
}
