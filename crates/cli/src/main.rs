use anyhow::Result;
use clap::{Parser, Subcommand};
use myserv_core::{
    model::{Credentials, Options, Server},
    provision, ssh,
};
use std::path::PathBuf;

#[derive(Parser)]
#[command(about = "MyServ: настройка собственного VPS через проверенный SSH")]
struct Cli {
    #[command(subcommand)]
    action: Action,
}
#[derive(Subcommand)]
enum Action {
    Fingerprint {
        ip: String,
        #[arg(default_value_t = 22)]
        port: u16,
    },
    Inspect {
        request: PathBuf,
    },
    Deploy {
        request: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
}
#[tokio::main]
async fn main() -> Result<()> {
    match Cli::parse().action {
        Action::Fingerprint { ip, port } => println!("{}", ssh::fingerprint(&ip, port).await?),
        Action::Inspect { request } => {
            let v: serde_json::Value = serde_json::from_slice(&std::fs::read(request)?)?;
            let server: Server = serde_json::from_value(v["server"].clone())?;
            let credentials: Credentials = serde_json::from_value(v["credentials"].clone())?;
            println!("{}", provision::inspect(&server, &credentials).await?);
        }
        Action::Deploy { request, output } => {
            let v: serde_json::Value = serde_json::from_slice(&std::fs::read(request)?)?;
            let server: Server = serde_json::from_value(v["server"].clone())?;
            let credentials: Credentials = serde_json::from_value(v["credentials"].clone())?;
            let options: Options = serde_json::from_value(v["options"].clone())?;
            let key = option_env!("MYSERV_ISSUER_PUBLIC_KEY")
                .ok_or_else(|| anyhow::anyhow!("Rebuild with MYSERV_ISSUER_PUBLIC_KEY"))?;
            let profile = provision::deploy(
                &server,
                &credentials,
                &options,
                v["license"].as_str().unwrap_or(""),
                key,
                |p| eprintln!("{}: {}", p.step, p.message),
            )
            .await?;
            let mut file = std::fs::OpenOptions::new();
            file.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                file.mode(0o600);
            }
            use std::io::Write;
            file.open(output)?.write_all(profile.content.as_bytes())?;
            println!("Конфигурация сохранена. Проверьте подключение VPN-клиентом.");
        }
    }
    Ok(())
}
