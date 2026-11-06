use crate::{
    license,
    model::{Credentials, Options, Profile, Progress, Server},
    profiles,
    ssh::Session,
};
use anyhow::{Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::json;

pub const PREFLIGHT: &str = include_str!("../../../infra/preflight.sh");
pub const INSTALL: &str = include_str!("../../../infra/install.sh");

pub async fn inspect(server: &Server, credentials: &Credentials) -> Result<String> {
    Session::connect(server, credentials)
        .await?
        .script(PREFLIGHT, |_| {})
        .await
}

pub async fn deploy(
    server: &Server,
    credentials: &Credentials,
    options: &Options,
    token: &str,
    issuer_key: &str,
    on_progress: impl Fn(Progress),
) -> Result<Profile> {
    server.validate()?;
    options.validate()?;
    license::verify(token, issuer_key, &server.binding()?)?;
    let session = Session::connect(server, credentials).await?;
    on_progress(Progress {
        step: "preflight".into(),
        message: "Проверяем Ubuntu, ресурсы и конфликты".into(),
    });
    session.script(PREFLIGHT, |_| {}).await?;
    let material = profiles::generate(server, options)?;
    let bundle = json!({
        "binding":server.binding()?, "material":material,
        "awg_dockerfile":include_str!("../../../infra/awg/Dockerfile"),
        "awg_entrypoint":include_str!("../../../infra/awg/entrypoint.sh"),
        "manifest":manifest(&options.protocol),
    });
    let script = format!(
        "MS_BUNDLE='{}'\n{}",
        STANDARD.encode(serde_json::to_vec(&bundle)?),
        INSTALL
    );
    session
        .script(&script, |step| {
            let message = match step {
                "packages" => "Устанавливаем Docker и системные пакеты",
                "kubernetes" => "Поднимаем K3s с Docker",
                "image" => "Подготавливаем контейнер протокола",
                "deploy" => "Настраиваем сервер и запускаем контейнер",
                "ready" => "Контейнер готов. Получаем конфигурацию",
                _ => "Настройка сервера",
            };
            on_progress(Progress {
                step: step.into(),
                message: message.into(),
            });
        })
        .await?;
    let data = session.script("python3 - <<'PY'\nimport json\nprint(json.dumps(json.load(open('/var/lib/myserv/state.json'))['material']['profile']))\nPY\n", |_| {}).await?;
    let profile: Profile = serde_json::from_str(&data)?;
    ensure!(!profile.content.is_empty(), "Сервер не вернул конфигурацию");
    Ok(profile)
}

pub fn manifest(protocol: &crate::model::Protocol) -> String {
    use crate::model::Protocol;
    let awg = matches!(protocol, Protocol::AmneziaWg);
    let container = if awg {
        json!({
            "name":"tunnel","image":"myserv-awg:3.1.20260828","imagePullPolicy":"Never",
            "securityContext":{"privileged":true},
            "volumeMounts":[{"name":"config","mountPath":"/etc/myserv","readOnly":true},{"name":"tun","mountPath":"/dev/net/tun"}],
            "readinessProbe":{"exec":{"command":["sh","-c","awg show awg0 public-key >/dev/null"]},"initialDelaySeconds":3,"periodSeconds":5},
            "resources":{"requests":{"memory":"64Mi","cpu":"100m"},"limits":{"memory":"512Mi","cpu":"1"}}
        })
    } else {
        json!({
            "name":"tunnel","image":"ghcr.io/xtls/xray-core:26.10.31","imagePullPolicy":"IfNotPresent",
            "args":["run","-config","/etc/myserv/server.json"],
            "securityContext":{"runAsUser":65532,"runAsNonRoot":true,"allowPrivilegeEscalation":false,"readOnlyRootFilesystem":true,"capabilities":{"drop":["ALL"],"add":["NET_BIND_SERVICE"]}},
            "volumeMounts":[{"name":"config","mountPath":"/etc/myserv","readOnly":true}],
            "readinessProbe":{"tcpSocket":{"host":"127.0.0.1","port":443},"initialDelaySeconds":3,"periodSeconds":5},
            "resources":{"requests":{"memory":"32Mi","cpu":"50m"},"limits":{"memory":"256Mi","cpu":"1"}}
        })
    };
    let mut volumes =
        vec![json!({"name":"config","secret":{"secretName":"tunnel","defaultMode":292}})];
    if awg {
        volumes.push(json!({"name":"tun","hostPath":{"path":"/dev/net/tun","type":"CharDevice"}}));
    }
    json!({"apiVersion":"apps/v1","kind":"Deployment","metadata":{"name":"tunnel","namespace":"myserv"},"spec":{"replicas":1,"strategy":{"type":"Recreate"},"selector":{"matchLabels":{"app":"myserv"}},"template":{"metadata":{"labels":{"app":"myserv"}},"spec":{"hostNetwork":true,"dnsPolicy":"Default","automountServiceAccountToken":false,"terminationGracePeriodSeconds":15,"containers":[container],"volumes":volumes}}}}).to_string()
}
