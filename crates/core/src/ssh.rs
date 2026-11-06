use crate::model::{Credentials, Server};
use anyhow::{Context, Result, ensure};
use russh::{
    ChannelMsg, client,
    keys::{HashAlg, PrivateKeyWithHashAlg, PublicKeyOrCertificate, decode_secret_key},
};
use std::{
    net::Ipv4Addr,
    sync::{Arc, Mutex},
    time::Duration,
};

struct HostKey {
    expected: Option<String>,
    observed: Arc<Mutex<Option<String>>>,
}
impl client::Handler for HostKey {
    type Error = russh::Error;
    async fn check_server_key(
        &mut self,
        key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let fingerprint = match key {
            PublicKeyOrCertificate::PublicKey { key, .. } => {
                key.fingerprint(HashAlg::Sha256).to_string()
            }
            PublicKeyOrCertificate::Certificate(_) => return Ok(false),
        };
        *self.observed.lock().expect("fingerprint lock") = Some(fingerprint.clone());
        Ok(self
            .expected
            .as_ref()
            .is_some_and(|expected| *expected == fingerprint))
    }
}

/// Unauthenticated discovery: deliberately rejects the host after recording its key.
/// The user must verify this fingerprint via the hosting console before authentication.
pub async fn fingerprint(ip: &str, port: u16) -> Result<String> {
    let _: Ipv4Addr = ip.parse().context("Укажите IPv4 сервера")?;
    ensure!(port > 0, "Неверный порт");
    let observed = Arc::new(Mutex::new(None));
    let handler = HostKey {
        expected: None,
        observed: observed.clone(),
    };
    let config = Arc::new(client::Config::default());
    let _ = tokio::time::timeout(
        Duration::from_secs(20),
        client::connect(config, (ip, port), handler),
    )
    .await;
    let result = observed.lock().expect("fingerprint lock").clone();
    result.context("Не удалось получить SSH-ключ: проверьте адрес, порт и доступность сервера")
}

pub struct Session(client::Handle<HostKey>);
impl Session {
    pub async fn connect(server: &Server, credentials: &Credentials) -> Result<Self> {
        server.validate()?;
        let config = Arc::new(client::Config {
            inactivity_timeout: Some(Duration::from_secs(1800)),
            keepalive_interval: Some(Duration::from_secs(15)),
            ..Default::default()
        });
        let handler = HostKey {
            expected: Some(server.fingerprint.clone()),
            observed: Arc::new(Mutex::new(None)),
        };
        let mut session = tokio::time::timeout(
            Duration::from_secs(20),
            client::connect(config, (server.ip.as_str(), server.port), handler),
        )
        .await
        .context("Таймаут SSH")?
        .context("SSH: соединение отклонено или изменился ключ сервера")?;
        let authenticated = tokio::time::timeout(Duration::from_secs(30), async {
            if !credentials.private_key.is_empty() {
                let passphrase =
                    (!credentials.passphrase.is_empty()).then_some(credentials.passphrase.as_str());
                let key = decode_secret_key(&credentials.private_key, passphrase)
                    .context("Не удалось открыть приватный SSH-ключ")?;
                let hash = session.best_supported_rsa_hash().await?.flatten();
                Ok::<_, anyhow::Error>(
                    session
                        .authenticate_publickey(
                            "root",
                            PrivateKeyWithHashAlg::new(Arc::new(key), hash),
                        )
                        .await?
                        .success(),
                )
            } else {
                ensure!(
                    !credentials.password.is_empty(),
                    "Укажите SSH-ключ или пароль root"
                );
                Ok(session
                    .authenticate_password("root", &credentials.password)
                    .await?
                    .success())
            }
        })
        .await
        .context("Таймаут авторизации SSH")??;
        ensure!(authenticated, "SSH: авторизация не пройдена");
        Ok(Self(session))
    }

    pub async fn script(&self, script: &str, on_step: impl Fn(&str)) -> Result<String> {
        tokio::time::timeout(Duration::from_secs(1800), async {
            let mut channel = self.0.channel_open_session().await?;
            channel.exec(true, "bash -se").await?;
            channel.data(script.as_bytes()).await?;
            channel.eof().await?;
            let mut out = Vec::new();
            let mut line = Vec::new();
            let mut exit = None;
            while let Some(msg) = channel.wait().await {
                match msg {
                    ChannelMsg::Data { data } => {
                        ensure!(out.len() + data.len() <= 4 * 1024 * 1024, "Слишком большой ответ SSH");
                        for b in data.iter() {
                            if *b == b'\n' {
                                if let Ok(s) = std::str::from_utf8(&line)
                                    && let Some(step) = s.strip_prefix("MYSERV:") { on_step(step); }
                                line.clear();
                            } else if line.len() < 1024 { line.push(*b); }
                        }
                        out.extend_from_slice(&data);
                    }
                    ChannelMsg::ExitStatus { exit_status } => exit = Some(exit_status),
                    _ => {} // stderr may contain secrets; never forward it to UI or telemetry.
                }
            }
            ensure!(exit == Some(0), "Команда сервера не завершилась успешно (код {:?}). Проверьте требования VPS и /var/log/myserv-setup.log через консоль хостинга; повторный запуск сохранит выданные ключи.", exit);
            Ok(String::from_utf8(out)?)
        }).await.context("Установка превысила 30 минут. Проверьте сервер перед повторным запуском")?
    }
}
