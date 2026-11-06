use crate::model::{Options, Profile, Protocol, Server};
use anyhow::Result;
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use qrcode::{QrCode, render::svg};
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize};
use serde_json::json;
use x25519_dalek::{PublicKey, StaticSecret};

// Stored only in root-owned server state. The client private key never enters logs.
#[derive(Serialize, Deserialize)]
pub struct Material {
    pub server_config: String,
    pub profile: Profile,
    pub protocol: Protocol,
}

pub fn qr(text: &str) -> Result<String> {
    Ok(QrCode::new(text)?
        .render::<svg::Color>()
        .min_dimensions(280, 280)
        .build())
}

pub fn generate(server: &Server, options: &Options) -> Result<Material> {
    server.validate()?;
    options.validate()?;
    let secret = StaticSecret::random_from_rng(OsRng);
    let public = PublicKey::from(&secret);
    match options.protocol {
        Protocol::AmneziaWg => {
            let client_secret = StaticSecret::random_from_rng(OsRng);
            let client_public = PublicKey::from(&client_secret);
            let mut header = [0u8; 32];
            OsRng.fill_bytes(&mut header);
            let mut psk = [0u8; 32];
            OsRng.fill_bytes(&mut psk);
            let params = format!(
                "Jc = 5\nJmin = 40\nJmax = 70\nS1 = 32\nS2 = 48\nS3 = 24\nS4 = 16\nH1 = 100000-199999\nH2 = 200000-299999\nH3 = 300000-399999\nH4 = 400000-499999\nHeaderProtectionKey = {}\nContentPaddingAddition = 0-32\n",
                STANDARD.encode(header)
            );
            let server_config = format!(
                "[Interface]\nPrivateKey = {}\nListenPort = 51820\n{}\n[Peer]\nPublicKey = {}\nPresharedKey = {}\nAllowedIPs = 10.78.0.2/32\n",
                STANDARD.encode(secret.to_bytes()),
                params,
                STANDARD.encode(client_public.as_bytes()),
                STANDARD.encode(psk)
            );
            let content = format!(
                "[Interface]\nPrivateKey = {}\nAddress = 10.78.0.2/32\nDNS = 1.1.1.1, 9.9.9.9\nMTU = 1280\n{}\n[Peer]\nPublicKey = {}\nPresharedKey = {}\nEndpoint = {}:51820\nAllowedIPs = 0.0.0.0/0, ::/0\nPersistentKeepalive = 25\n",
                STANDARD.encode(client_secret.to_bytes()),
                params,
                STANDARD.encode(public.as_bytes()),
                STANDARD.encode(psk),
                server.ip
            );
            Ok(Material {
                server_config,
                protocol: Protocol::AmneziaWg,
                profile: Profile {
                    filename: "myserv-awg.conf".into(),
                    qr_svg: qr(&content)?,
                    content,
                    import_uri: None,
                },
            })
        }
        Protocol::VlessReality => {
            let id = uuid::Uuid::new_v4().to_string();
            let mut short = [0u8; 8];
            OsRng.fill_bytes(&mut short);
            let short: String = short.iter().map(|b| format!("{b:02x}")).collect();
            let public = URL_SAFE_NO_PAD.encode(public.as_bytes());
            let server_config = serde_json::to_string_pretty(&json!({
                "log":{"loglevel":"warning","access":"none"},
                "inbounds":[{"listen":"0.0.0.0","port":443,"protocol":"vless","settings":{"clients":[{"id":id,"flow":"xtls-rprx-vision"}],"decryption":"none"},"streamSettings":{"network":"tcp","security":"reality","realitySettings":{"show":false,"target":format!("{}:443",options.reality_domain),"xver":0,"serverNames":[options.reality_domain],"privateKey":URL_SAFE_NO_PAD.encode(secret.to_bytes()),"shortIds":[short]}}}],
                "outbounds":[{"protocol":"freedom","tag":"direct"},{"protocol":"blackhole","tag":"blocked"}],
                "routing":{"rules":[{"type":"field","ip":["10.0.0.0/8","172.16.0.0/12","192.168.0.0/16","127.0.0.0/8","169.254.0.0/16","::1/128","fc00::/7","fe80::/10"],"outboundTag":"blocked"}]}
            }))?;
            let content = serde_json::to_string_pretty(&json!({
                "log":{"loglevel":"warning"},
                "inbounds":[{"listen":"127.0.0.1","port":10808,"protocol":"socks","settings":{"udp":true}}],
                "outbounds":[{"protocol":"vless","settings":{"vnext":[{"address":server.ip,"port":443,"users":[{"id":id,"encryption":"none","flow":"xtls-rprx-vision"}]}]},"streamSettings":{"network":"tcp","security":"reality","realitySettings":{"fingerprint":"chrome","serverName":options.reality_domain,"password":public,"shortId":short}}}]
            }))?;
            let mut uri = url::Url::parse(&format!("vless://{id}@{}:443", server.ip))?;
            uri.query_pairs_mut().extend_pairs([
                ("encryption", "none"),
                ("security", "reality"),
                ("type", "tcp"),
                ("flow", "xtls-rprx-vision"),
                ("sni", options.reality_domain.as_str()),
                ("fp", "chrome"),
                ("pbk", public.as_str()),
                ("sid", short.as_str()),
            ]);
            uri.set_fragment(Some("MyServ"));
            let uri = uri.to_string();
            Ok(Material {
                server_config,
                protocol: Protocol::VlessReality,
                profile: Profile {
                    filename: "myserv-xray.json".into(),
                    content,
                    qr_svg: qr(&uri)?,
                    import_uri: Some(uri),
                },
            })
        }
    }
}
