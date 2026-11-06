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
/// The GUI pins it on first use; CLI callers may verify it independently.
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

pub struct Session {
    handle: client::Handle<HostKey>,
    sudo_password: Option<zeroize::Zeroizing<String>>,
}

// Keep the parent bash alive: sudo's non-TTY timestamp is bound to its parent PID.
// Without the trailing exit, bash may exec sudo and invalidate that timestamp.
pub const SUDO_COMMAND: &str = "bash -c 'IFS= read -r myserv_pw; printf \"%s\\n\" \"$myserv_pw\" | sudo -S -p \"\" -v || exit 77; unset myserv_pw; sudo -n -- bash -se; exit $?'";

impl Session {
    pub async fn connect(server: &Server, credentials: &Credentials) -> Result<Self> {
        credentials.validate()?;
        Self::authenticate(Self::open(server).await?, credentials).await
    }

    pub async fn connect_remembered(
        ip: &str,
        port: u16,
        credentials: &Credentials,
        dir: &std::path::Path,
    ) -> Result<(Server, Self)> {
        credentials.validate()?;
        let server = match crate::trust::load(dir, ip, port)? {
            Some(server) => server,
            None => Server {
                ip: ip.into(),
                port,
                fingerprint: fingerprint(ip, port).await?,
            },
        };
        let handle = Self::open(&server).await?;
        // Finish the signed handshake and persist the key before sending credentials.
        crate::trust::remember(dir, &server)?;
        let session = Self::authenticate(handle, credentials).await?;
        Ok((server, session))
    }

    async fn open(server: &Server) -> Result<client::Handle<HostKey>> {
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
        let session = tokio::time::timeout(
            Duration::from_secs(20),
            client::connect(config, (server.ip.as_str(), server.port), handler),
        )
        .await
        .context("Таймаут SSH")?
        .context("Не удалось безопасно подключиться к серверу. Проверьте IP и порт. Если ключ сервера изменился после переустановки VPS, обратитесь в поддержку для обновления записи сервера")?;
        Ok(session)
    }

    async fn authenticate(
        mut session: client::Handle<HostKey>,
        credentials: &Credentials,
    ) -> Result<Self> {
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
                            &credentials.username,
                            PrivateKeyWithHashAlg::new(Arc::new(key), hash),
                        )
                        .await?
                        .success(),
                )
            } else {
                ensure!(!credentials.password.is_empty(), "Введите пароль сервера");
                Ok(session
                    .authenticate_password(&credentials.username, &credentials.password)
                    .await?
                    .success())
            }
        })
        .await
        .context("Таймаут авторизации SSH")??;
        ensure!(
            authenticated,
            "Не удалось войти на сервер. Проверьте логин и пароль. На сервере должен быть разрешён вход по паролю"
        );
        Ok(Self {
            handle: session,
            sudo_password: (credentials.username != "root")
                .then(|| zeroize::Zeroizing::new(credentials.password.clone())),
        })
    }

    pub async fn script(&self, script: &str, on_step: impl Fn(&str)) -> Result<String> {
        tokio::time::timeout(Duration::from_secs(1800), async {
            let mut channel = self.handle.channel_open_session().await?;
            channel
                .exec(
                    true,
                    if self.sudo_password.is_some() {
                        SUDO_COMMAND
                    } else {
                        "bash -se"
                    },
                )
                .await?;
            let input = zeroize::Zeroizing::new(match &self.sudo_password {
                Some(password) => format!("{}\n{script}", password.as_str()),
                None => script.to_owned(),
            });
            channel.data(input.as_bytes()).await?;
            channel.eof().await?;
            let mut out = Vec::new();
            let mut line = Vec::new();
            let mut exit = None;
            while let Some(msg) = channel.wait().await {
                match msg {
                    ChannelMsg::Data { data } => {
                        ensure!(
                            out.len() + data.len() <= 4 * 1024 * 1024,
                            "Слишком большой ответ SSH"
                        );
                        for b in data.iter() {
                            if *b == b'\n' {
                                if let Ok(s) = std::str::from_utf8(&line)
                                    && let Some(step) = s.strip_prefix("MYSERV:")
                                {
                                    on_step(step);
                                }
                                line.clear();
                            } else if line.len() < 1024 {
                                line.push(*b);
                            }
                        }
                        out.extend_from_slice(&data);
                    }
                    ChannelMsg::ExitStatus { exit_status } => exit = Some(exit_status),
                    _ => {} // stderr may contain secrets; never forward it to UI or telemetry.
                }
            }
            if exit != Some(0) {
                anyhow::bail!(friendly_failure(&String::from_utf8_lossy(&out), exit));
            }
            Ok(String::from_utf8(out)?)
        })
        .await
        .context("Установка превысила 30 минут. Проверьте сервер перед повторным запуском")?
    }
}

pub fn friendly_failure(output: &str, exit: Option<u32>) -> String {
    let code = output
        .lines()
        .find_map(|line| line.strip_prefix("MYSERV_ERROR:"));
    let message = match code {
        Some("install-locked") => {
            "На сервере уже идёт установка MyServ. Дождитесь её завершения и подключитесь снова"
        }
        Some("install-prepare") => {
            "Не удалось открыть журнал или служебные файлы установки. Проверьте свободное место и права пользователя"
        }
        Some("install-state") => {
            "Не удалось прочитать или сохранить настройки MyServ. Если ранее выбран другой протокол, повторите установку с ним. Сохранённые ключи не удаляйте"
        }
        Some("install-packages") => {
            "Не удалось установить системные пакеты, Docker или Buildx. Подробности находятся в /var/log/myserv-setup.log на сервере"
        }
        Some("install-tun") => {
            "Не удалось включить сетевой интерфейс VPN. Проверьте, что хостинг предоставляет /dev/net/tun и разрешает IP forwarding"
        }
        Some("install-reality") => {
            "Проверка домена REALITY не прошла. Нужен доступный с сервера домен с TLS 1.3, HTTP/2 и действительным сертификатом"
        }
        Some("install-kubernetes-download") => {
            "Не удалось скачать или проверить Kubernetes K3s. Проверьте доступ сервера к GitHub; подробности — /var/log/myserv-setup.log"
        }
        Some("install-kubernetes-start") => {
            "Kubernetes K3s не запустился. Подробности — /var/log/myserv-setup.log и журнал службы myserv-k3s на сервере"
        }
        Some("install-image") => {
            "Не удалось собрать или загрузить контейнер VPN. Подробности — /var/log/myserv-setup.log; проверьте доступ сервера к реестрам контейнеров и свободное место"
        }
        Some("install-deploy") => {
            "Контейнер VPN не вышел в готовое состояние. Подробности — /var/log/myserv-setup.log и состояние контейнеров myserv в Kubernetes"
        }
        Some("root") => {
            "У этого пользователя нет прав администратора. Используйте root или пользователя с правами sudo"
        }
        Some("systemd") => {
            "Этот тип сервера не подходит. Нужен полноценный VPS с Ubuntu 24.04, а не контейнер хостинга"
        }
        Some("os") => {
            "На сервере нужна Ubuntu 24.04 LTS. Выберите эту систему в панели хостинга или используйте другой VPS"
        }
        Some("arch") => "Архитектура сервера не поддерживается. Выберите VPS x86_64 или ARM64",
        Some("memory") => {
            "На сервере недостаточно памяти. Для установки требуется тариф с 2 ГБ RAM или больше"
        }
        Some("disk") => {
            "На сервере недостаточно свободного места. Для первой установки нужно не менее 8 ГБ"
        }
        Some("python") => {
            "В образе сервера отсутствует Python 3. Используйте стандартный образ Ubuntu 24.04 от хостинга"
        }
        Some("kubernetes") => {
            "На сервере уже установлен Kubernetes. Для этой версии приложения нужен отдельный чистый VPS"
        }
        Some("ports") => {
            "Порт 443 или 51820 уже занят другим приложением. Используйте чистый VPS или освободите эти порты"
        }
        Some("containers") => {
            "На сервере уже есть Docker-контейнеры. Чтобы сохранить их, используйте отдельный чистый VPS"
        }
        Some("checks") => {
            "Не удалось проверить ресурсы сервера. Используйте стандартный образ Ubuntu 24.04 или обратитесь в поддержку"
        }
        _ if exit == Some(77) => {
            "Пользователь не получил права администратора. Используйте root или логин с правами sudo и его пароль"
        }
        _ => {
            "Операция на сервере остановлена. Повторите подключение; если ошибка сохраняется, обратитесь в поддержку. Существующие ключи VPN сохранятся"
        }
    };
    // Only numeric diagnostics cross the UI boundary; raw logs may contain secrets.
    let details = output
        .lines()
        .find_map(|line| line.strip_prefix("MYSERV_DETAIL:"))
        .and_then(|s| s.split_once(':'))
        .and_then(|(line, status)| Some((line.parse::<u32>().ok()?, status.parse::<u32>().ok()?)));
    match details {
        Some((line, status)) => format!("{message}. Код {status}, строка установщика {line}"),
        None => message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::friendly_failure;

    #[test]
    fn installer_error_reports_stage_and_numeric_location_without_raw_logs() {
        let output = "private-secret\nMYSERV_ERROR:install-image\nMYSERV_DETAIL:124:42\n";
        let message = friendly_failure(output, Some(42));
        assert!(message.contains("контейнер VPN"));
        assert!(message.contains("Код 42, строка установщика 124"));
        assert!(!message.contains("private-secret"));
        assert!(!friendly_failure("MYSERV_DETAIL:secret:password", Some(1)).contains("password"));
        assert!(friendly_failure("MYSERV_ERROR:install-locked\n", Some(1)).contains("уже идёт"));
    }
}
