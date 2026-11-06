use myserv_core::{
    model::{Credentials, Server as Target},
    ssh,
};
use russh::{
    Channel, ChannelId,
    server::{self, Server as _, Session},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
#[derive(Clone)]
struct Fixture {
    authentications: Arc<AtomicUsize>,
    data: Vec<u8>,
}
impl server::Server for Fixture {
    type Handler = Self;
    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> Self {
        Self {
            authentications: self.authentications.clone(),
            data: vec![],
        }
    }
}
impl server::Handler for Fixture {
    type Error = russh::Error;
    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<server::Auth, Self::Error> {
        self.authentications.fetch_add(1, Ordering::SeqCst);
        Ok(if user == "root" && password == "test-only" {
            server::Auth::Accept
        } else {
            server::Auth::reject()
        })
    }
    async fn channel_open_session(
        &mut self,
        _: Channel<server::Msg>,
        reply: server::ChannelOpenHandle,
        _: &mut Session,
    ) -> Result<(), Self::Error> {
        reply.accept().await;
        Ok(())
    }
    async fn exec_request(
        &mut self,
        c: ChannelId,
        data: &[u8],
        s: &mut Session,
    ) -> Result<(), Self::Error> {
        assert_eq!(data, b"bash -se");
        s.channel_success(c)?;
        Ok(())
    }
    async fn data(
        &mut self,
        _: ChannelId,
        data: &[u8],
        _: &mut Session,
    ) -> Result<(), Self::Error> {
        self.data.extend_from_slice(data);
        Ok(())
    }
    async fn channel_eof(&mut self, c: ChannelId, s: &mut Session) -> Result<(), Self::Error> {
        let input = std::mem::take(&mut self.data);
        let output = if String::from_utf8_lossy(&input)
            .contains("open('/var/lib/myserv/state.json')")
        {
            br#"{"filename":"myserv-awg.conf","content":"fixture-profile","import_uri":null,"qr_svg":"<svg/>"}"#.to_vec()
        } else {
            b"MYSERV:ready\nfixture-output\n".to_vec()
        };
        s.data(c, output)?;
        s.exit_status_request(c, if input == b"fail" { 1 } else { 0 })?;
        s.eof(c)?;
        s.close(c)?;
        Ok(())
    }
}
#[tokio::test]
async fn pins_host_before_auth_and_preserves_remote_exit_status() {
    let key = russh::keys::PrivateKey::random(
        &mut russh::keys::key::safe_rng(),
        russh::keys::Algorithm::Ed25519,
    )
    .unwrap();
    let expected = key
        .public_key()
        .fingerprint(russh::keys::HashAlg::Sha256)
        .to_string();
    let config = Arc::new(server::Config {
        keys: vec![key],
        auth_rejection_time: Duration::ZERO,
        ..Default::default()
    });
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut fixture = Fixture {
        authentications: calls.clone(),
        data: vec![],
    };
    let task = tokio::spawn(async move { fixture.run_on_socket(config, &listener).await.unwrap() });
    assert_eq!(ssh::fingerprint("127.0.0.1", port).await.unwrap(), expected);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let credentials = Credentials {
        username: "root".into(),
        password: "test-only".into(),
        private_key: String::new(),
        passphrase: String::new(),
    };
    let mut target = Target {
        ip: "127.0.0.1".into(),
        port,
        fingerprint: format!("SHA256:{}", "a".repeat(43)),
    };
    assert!(ssh::Session::connect(&target, &credentials).await.is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    target.fingerprint = expected;
    let session = ssh::Session::connect(&target, &credentials).await.unwrap();
    assert!(
        session
            .script("success", |step| assert_eq!(step, "ready"))
            .await
            .unwrap()
            .contains("fixture-output")
    );
    let session = ssh::Session::connect(&target, &credentials).await.unwrap();
    assert!(session.script("fail", |_| {}).await.is_err());
    let bad = Credentials {
        username: "root".into(),
        password: "wrong".into(),
        private_key: String::new(),
        passphrase: String::new(),
    };
    assert!(ssh::Session::connect(&target, &bad).await.is_err());
    let dir = std::env::temp_dir().join(format!("myserv-ssh-{}", uuid::Uuid::new_v4()));
    let (remembered, first) =
        ssh::Session::connect_remembered(&target.ip, port, &credentials, &dir)
            .await
            .unwrap();
    assert_eq!(remembered.fingerprint, target.fingerprint);
    first.script("success", |_| {}).await.unwrap();
    let (_, repeat) = ssh::Session::connect_remembered(&target.ip, port, &credentials, &dir)
        .await
        .unwrap();
    repeat.script("success", |_| {}).await.unwrap();
    let options = myserv_core::model::Options {
        protocol: myserv_core::model::Protocol::AmneziaWg,
        reality_domain: String::new(),
    };
    let before = calls.load(Ordering::SeqCst);
    // Paid entry point continues to require a license even in free-enabled builds.
    assert!(
        myserv_core::provision::deploy(&target, &credentials, &options, "", "", |_| {})
            .await
            .is_err()
    );
    assert_eq!(calls.load(Ordering::SeqCst), before);
    #[cfg(feature = "free-preview")]
    {
        let profile =
            myserv_core::provision::deploy_preview(&target, &credentials, &options, |_| {})
                .await
                .unwrap();
        assert_eq!(profile.content, "fixture-profile");
        assert_eq!(calls.load(Ordering::SeqCst), before + 1);
    }
    // A different saved key must stop the handshake before sending any password.
    let mut changed = remembered.clone();
    changed.fingerprint = format!("SHA256:{}", "b".repeat(43));
    std::fs::write(
        dir.join(format!("127.0.0.1-{port}.json")),
        serde_json::to_vec(&changed).unwrap(),
    )
    .unwrap();
    let before = calls.load(Ordering::SeqCst);
    assert!(
        ssh::Session::connect_remembered(&target.ip, port, &credentials, &dir)
            .await
            .is_err()
    );
    assert_eq!(calls.load(Ordering::SeqCst), before);
    std::fs::remove_dir_all(dir).unwrap();
    task.abort();
}

/// Run against the disposable OpenSSH container described in scripts/ssh-smoke.sh.
#[tokio::test]
#[ignore = "requires disposable OpenSSH fixture"]
async fn openssh_root_and_sudo() {
    let port: u16 = std::env::var("MYSERV_TEST_SSH_PORT")
        .unwrap()
        .parse()
        .unwrap();
    let dir = std::env::temp_dir().join(format!("myserv-openssh-{}", uuid::Uuid::new_v4()));
    for username in ["root", "ubuntu", "limited"] {
        let credentials = Credentials {
            username: username.into(),
            password: "test-only-$'\\!".into(),
            private_key: String::new(),
            passphrase: String::new(),
        };
        let (_, session) = ssh::Session::connect_remembered("127.0.0.1", port, &credentials, &dir)
            .await
            .unwrap();
        let result = session
            .script("id -u\nprintf 'script-intact\\n'\n", |_| {})
            .await;
        if username == "limited" {
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("права администратора")
            );
        } else {
            assert_eq!(
                result.unwrap_or_else(|e| panic!("{username}: {e}")),
                "0\nscript-intact\n"
            );
            let failure = session
                .script("printf 'MYSERV_ERROR:memory\\n'; exit 1", |_| {})
                .await
                .unwrap_err()
                .to_string();
            assert!(failure.contains("2 ГБ"), "{failure}");
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}
