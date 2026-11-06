use anyhow::Result;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::args().nth(1).as_deref() == Some("keygen") {
        let key = SigningKey::generate(&mut OsRng);
        // Explicit operator command only. Redirect to a secure secrets store, never CI logs.
        println!(
            "MYSERV_SIGNING_KEY={}\nMYSERV_ISSUER_PUBLIC_KEY={}",
            URL_SAFE_NO_PAD.encode(key.to_bytes()),
            URL_SAFE_NO_PAD.encode(key.verifying_key().to_bytes())
        );
        return Ok(());
    }
    let service = std::sync::Arc::new(myserv_billing::Service::from_env()?);
    let addr = std::env::var("MYSERV_LISTEN").unwrap_or("127.0.0.1:8090".into());
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("MyServ billing listening on {addr}");
    axum::serve(listener, myserv_billing::router(service)).await?;
    Ok(())
}
