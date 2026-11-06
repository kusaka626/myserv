use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::net::Ipv4Addr;
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Clone, Serialize, Deserialize)]
pub struct Server {
    pub ip: String,
    pub port: u16,
    pub fingerprint: String,
}

impl Server {
    pub fn validate(&self) -> Result<()> {
        let ip: Ipv4Addr = self.ip.parse()?;
        ensure!(
            !ip.is_unspecified() && !ip.is_multicast() && !ip.is_broadcast(),
            "Неверный IPv4 сервера"
        );
        ensure!(self.port > 0, "SSH-порт не может быть 0");
        ensure!(
            self.fingerprint.starts_with("SHA256:") && self.fingerprint.len() == 50,
            "Подтвердите SHA256-отпечаток SSH-ключа сервера"
        );
        Ok(())
    }
    pub fn binding(&self) -> Result<String> {
        self.validate()?;
        Ok(format!(
            "{:x}",
            Sha256::digest(format!("myserv-v1\n{}\n{}", self.ip, self.fingerprint))
        ))
    }
}

// Never derive Debug or Serialize for credentials. No credentials reach billing.
#[derive(Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct Credentials {
    pub password: String,
    pub private_key: String,
    pub passphrase: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Protocol {
    AmneziaWg,
    VlessReality,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Options {
    pub protocol: Protocol,
    pub reality_domain: String,
}

impl Options {
    pub fn validate(&self) -> Result<()> {
        if matches!(self.protocol, Protocol::VlessReality) {
            let d = &self.reality_domain;
            ensure!(
                d.len() <= 253 && d.contains('.') && d.parse::<Ipv4Addr>().is_err(),
                "Нужен домен TLS 1.3 для REALITY"
            );
            ensure!(
                d.split('.').all(|s| !s.is_empty()
                    && s.len() <= 63
                    && !s.starts_with('-')
                    && !s.ends_with('-')
                    && s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')),
                "Неверный домен REALITY"
            );
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
pub struct Profile {
    pub filename: String,
    pub content: String,
    pub import_uri: Option<String>,
    pub qr_svg: String,
}

#[derive(Clone, Serialize)]
pub struct Progress {
    pub step: String,
    pub message: String,
}
