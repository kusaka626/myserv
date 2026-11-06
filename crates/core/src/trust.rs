//! Trust on first use: only public host fingerprints are stored, never credentials.
use crate::model::Server;
use anyhow::{Context, Result, ensure};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

fn path(dir: &Path, ip: &str, port: u16) -> Result<PathBuf> {
    let ip: std::net::Ipv4Addr = ip.parse().context("Укажите IPv4 сервера")?;
    ensure!(port > 0, "Укажите порт от 1 до 65535");
    Ok(dir.join(format!("{ip}-{port}.json")))
}

pub fn load(dir: &Path, ip: &str, port: u16) -> Result<Option<Server>> {
    let file = path(dir, ip, port)?;
    let bytes = match fs::read(file) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).context("Не удалось прочитать сохранённый ключ сервера"),
    };
    let server: Server = serde_json::from_slice(&bytes)
        .context("Повреждена локальная запись сервера. Обратитесь в поддержку")?;
    server.validate()?;
    ensure!(
        server.ip == ip && server.port == port,
        "Локальная запись относится к другому серверу"
    );
    Ok(Some(server))
}

pub fn remember(dir: &Path, server: &Server) -> Result<()> {
    server.validate()?;
    fs::create_dir_all(dir).context("Не удалось сохранить ключ сервера на устройстве")?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(path(dir, &server.ip, server.port)?) {
        Ok(mut file) => {
            file.write_all(&serde_json::to_vec(server)?)?;
            file.sync_all()?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let old =
                load(dir, &server.ip, server.port)?.context("Нет сохранённого ключа сервера")?;
            ensure!(
                old.fingerprint == server.fingerprint,
                "Ключ сервера изменился. Подключение остановлено. Если вы переустановили VPS, обратитесь в поддержку для обновления записи сервера"
            );
        }
        Err(e) => return Err(e).context("Не удалось сохранить ключ сервера на устройстве"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn remembers_first_key_and_rejects_replacement() {
        let dir = std::env::temp_dir().join(format!("myserv-trust-test-{}", uuid::Uuid::new_v4()));
        let mut server = Server {
            ip: "192.0.2.1".into(),
            port: 22,
            fingerprint: format!("SHA256:{}", "a".repeat(43)),
        };
        assert!(load(&dir, &server.ip, server.port).unwrap().is_none());
        remember(&dir, &server).unwrap();
        assert_eq!(
            load(&dir, &server.ip, server.port)
                .unwrap()
                .unwrap()
                .fingerprint,
            server.fingerprint
        );
        remember(&dir, &server).unwrap();
        server.fingerprint = format!("SHA256:{}", "b".repeat(43));
        assert!(remember(&dir, &server).is_err());
        std::fs::remove_file(path(&dir, &server.ip, server.port).unwrap()).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
