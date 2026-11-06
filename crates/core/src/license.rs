use anyhow::{Result, ensure};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD as B64};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Grant {
    pub product: String,
    pub order_id: String,
    pub server_binding: String,
    pub amount_kopeks: u32,
    pub currency: String,
}

pub fn sign(grant: &Grant, key: &SigningKey) -> Result<String> {
    let bytes = serde_json::to_vec(grant)?;
    Ok(format!(
        "{}.{}",
        B64.encode(&bytes),
        B64.encode(key.sign(&bytes).to_bytes())
    ))
}

pub fn verify(token: &str, public_key: &str, binding: &str) -> Result<Grant> {
    ensure!(token.len() < 4096, "Неверная лицензия");
    let (payload, sig) = token
        .split_once('.')
        .ok_or_else(|| anyhow::anyhow!("Нет подписи лицензии"))?;
    let key: [u8; 32] = B64
        .decode(public_key)?
        .try_into()
        .map_err(|_| anyhow::anyhow!("Неверный ключ издателя"))?;
    let bytes = B64.decode(payload)?;
    VerifyingKey::from_bytes(&key)?
        .verify_strict(&bytes, &Signature::from_slice(&B64.decode(sig)?)?)?;
    let grant: Grant = serde_json::from_slice(&bytes)?;
    ensure!(
        grant.product == crate::PRODUCT
            && grant.amount_kopeks == crate::PRICE_KOPEKS
            && grant.currency == "RUB",
        "Лицензия другого продукта"
    );
    ensure!(
        grant.server_binding == binding,
        "Оплата относится к другому серверу"
    );
    Ok(grant)
}
