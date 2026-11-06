// Generates disposable test material. Never include these artifacts in a release.
use myserv_core::{
    model::{Options, Protocol, Server},
    profiles,
};
fn main() -> anyhow::Result<()> {
    let dir = std::env::args().nth(1).expect("output directory");
    std::fs::create_dir_all(&dir)?;
    let s = Server {
        ip: "203.0.113.10".into(),
        port: 22,
        fingerprint: format!("SHA256:{}", "a".repeat(43)),
    };
    for (name, p) in [
        ("awg", Protocol::AmneziaWg),
        ("xray", Protocol::VlessReality),
    ] {
        let m = profiles::generate(
            &s,
            &Options {
                protocol: p,
                reality_domain: "www.microsoft.com".into(),
            },
        )?;
        std::fs::write(format!("{dir}/{name}-server"), m.server_config)?;
        std::fs::write(format!("{dir}/{name}-client"), m.profile.content)?;
    }
    Ok(())
}
