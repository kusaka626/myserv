use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use ed25519_dalek::SigningKey;
use myserv_core::{
    license::{self, Grant},
    model::{Options, Protocol, Server},
    profiles, provision,
};
use rand::rngs::OsRng;
use serde_json::Value;
fn server() -> Server {
    Server {
        ip: "203.0.113.10".into(),
        port: 22,
        fingerprint: format!("SHA256:{}", "a".repeat(43)),
    }
}
fn options(protocol: Protocol) -> Options {
    Options {
        protocol,
        reality_domain: "www.microsoft.com".into(),
    }
}
fn value<'a>(text: &'a str, key: &str) -> &'a str {
    text.lines()
        .find_map(|l| l.strip_prefix(&format!("{key} = ")))
        .unwrap()
}
#[test]
fn awg_key_pairs_and_obfuscation_match() {
    let m = profiles::generate(&server(), &options(Protocol::AmneziaWg)).unwrap();
    let client = &m.profile.content;
    let raw: [u8; 32] = STANDARD
        .decode(value(client, "PrivateKey"))
        .unwrap()
        .try_into()
        .unwrap();
    let public = x25519_dalek::PublicKey::from(&x25519_dalek::StaticSecret::from(raw));
    assert_eq!(
        STANDARD.encode(public.as_bytes()),
        value(&m.server_config, "PublicKey")
    );
    for key in [
        "HeaderProtectionKey",
        "PresharedKey",
        "S1",
        "S2",
        "S3",
        "S4",
        "H1",
        "H2",
        "H3",
        "H4",
    ] {
        assert_eq!(value(client, key), value(&m.server_config, key));
    }
    assert!(client.contains("AllowedIPs = 0.0.0.0/0, ::/0"));
    assert!(m.profile.qr_svg.contains("<svg"));
    assert_ne!(
        profiles::generate(&server(), &options(Protocol::AmneziaWg))
            .unwrap()
            .profile
            .content,
        *client
    );
}
#[test]
fn reality_configs_and_uri_have_matching_credentials() {
    let m = profiles::generate(&server(), &options(Protocol::VlessReality)).unwrap();
    let s: Value = serde_json::from_str(&m.server_config).unwrap();
    let c: Value = serde_json::from_str(&m.profile.content).unwrap();
    assert_eq!(
        s["inbounds"][0]["settings"]["clients"][0]["id"],
        c["outbounds"][0]["settings"]["vnext"][0]["users"][0]["id"]
    );
    let r = &s["inbounds"][0]["streamSettings"]["realitySettings"];
    let raw: [u8; 32] = URL_SAFE_NO_PAD
        .decode(r["privateKey"].as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    let public = x25519_dalek::PublicKey::from(&x25519_dalek::StaticSecret::from(raw));
    assert_eq!(
        URL_SAFE_NO_PAD.encode(public.as_bytes()),
        c["outbounds"][0]["streamSettings"]["realitySettings"]["password"]
    );
    let uri = url::Url::parse(m.profile.import_uri.as_ref().unwrap()).unwrap();
    assert_eq!(uri.scheme(), "vless");
    assert!(
        uri.query_pairs()
            .any(|(k, v)| k == "pbk" && v == URL_SAFE_NO_PAD.encode(public.as_bytes()))
    );
}
#[test]
fn malformed_hosts_and_domains_cannot_become_shell_commands() {
    let mut s = server();
    s.ip = "1.2.3.4; rm -rf /".into();
    assert!(s.validate().is_err());
    for domain in [
        "example.com;id",
        "$(whoami).org",
        "a\nb.com",
        "-bad.example",
        "1.2.3.4",
        "a..com",
    ] {
        let mut o = options(Protocol::VlessReality);
        o.reality_domain = domain.into();
        assert!(o.validate().is_err(), "{domain}");
    }
}
#[test]
fn license_rejects_tampering_wrong_server_and_wrong_issuer() {
    let key = SigningKey::generate(&mut OsRng);
    let public = URL_SAFE_NO_PAD.encode(key.verifying_key().to_bytes());
    let grant = Grant {
        product: myserv_core::PRODUCT.into(),
        order_id: uuid::Uuid::new_v4().to_string(),
        server_binding: server().binding().unwrap(),
        amount_kopeks: 19900,
        currency: "RUB".into(),
    };
    let token = license::sign(&grant, &key).unwrap();
    assert!(license::verify(&token, &public, &grant.server_binding).is_ok());
    assert!(license::verify(&token, &public, "another server").is_err());
    let mut tampered = token.into_bytes();
    tampered[8] = if tampered[8] == b'A' { b'B' } else { b'A' };
    assert!(
        license::verify(
            std::str::from_utf8(&tampered).unwrap(),
            &public,
            &grant.server_binding
        )
        .is_err()
    );
    let token = license::sign(&grant, &SigningKey::generate(&mut OsRng)).unwrap();
    assert!(license::verify(&token, &public, &grant.server_binding).is_err());
}
#[test]
fn manifests_limit_replicas_and_expose_only_protocol_resources() {
    for p in [Protocol::AmneziaWg, Protocol::VlessReality] {
        let m: Value = serde_json::from_str(&provision::manifest(&p)).unwrap();
        assert_eq!(m["spec"]["replicas"], 1);
        assert_eq!(m["spec"]["strategy"]["type"], "Recreate");
        assert_eq!(
            m["spec"]["template"]["spec"]["automountServiceAccountToken"],
            false
        );
    }
}
