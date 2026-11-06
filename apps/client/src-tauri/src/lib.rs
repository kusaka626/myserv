use myserv_core::{
    billing::{Billing, Order, Payment},
    model::{Credentials, Options, Profile, Server},
    provision, ssh,
};
use rand::RngCore;
use serde::Serialize;
use std::{collections::HashMap, sync::Arc};
use tauri::{Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_fs::FsExt;
use tauri_plugin_opener::OpenerExt;
use tokio::sync::Mutex;
use uuid::Uuid;

struct Connection {
    server: Server,
    credentials: Credentials,
    order: Option<(Order, String)>,
}
#[derive(Default)]
struct AppState {
    sessions: Mutex<HashMap<String, Arc<Mutex<Connection>>>>,
    operation: Mutex<()>,
}
#[derive(Serialize)]
struct Connected {
    session: String,
    summary: String,
}
#[derive(Serialize)]
struct Settings {
    free_preview: bool,
    billing_ready: bool,
    mobile: bool,
    version: &'static str,
}
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn billing() -> Result<Billing, String> {
    if cfg!(feature = "free-preview") {
        return Err("В тестовой сборке оплата отключена. Настройка бесплатна".into());
    }
    Billing::new(
        option_env!("MYSERV_BILLING_URL").ok_or("Платёжный сервис не настроен в этой сборке")?,
    )
    .map_err(err)
}
#[cfg(not(feature = "free-preview"))]
fn issuer() -> Result<&'static str, String> {
    option_env!("MYSERV_ISSUER_PUBLIC_KEY").ok_or("Ключ издателя не настроен в этой сборке".into())
}
async fn session(state: &AppState, id: &str) -> Result<Arc<Mutex<Connection>>, String> {
    state
        .sessions
        .lock()
        .await
        .get(id)
        .cloned()
        .ok_or("SSH-сеанс завершён. Подключитесь снова".into())
}

fn order_path(app: &tauri::AppHandle, binding: &str) -> Result<std::path::PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(err)?.join("orders");
    std::fs::create_dir_all(&dir).map_err(err)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).map_err(err)?;
    }
    Ok(dir.join(format!("{binding}.json")))
}

#[tauri::command]
async fn export_file(
    app: tauri::AppHandle,
    filename: String,
    content: String,
) -> Result<bool, String> {
    if !["myserv-awg.conf", "myserv-xray.json", "myserv-license.txt"].contains(&filename.as_str())
        || content.len() > 65536
    {
        return Err("Неверный файл экспорта".into());
    }
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_file_name(filename)
        .save_file(move |path| {
            let _ = tx.send(path);
        });
    let Some(path) = rx.await.map_err(err)? else {
        return Ok(false);
    };
    let mut options = tauri_plugin_fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    use std::io::Write;
    let mut file = app.fs().open(path, options).map_err(err)?;
    file.write_all(content.as_bytes()).map_err(err)?;
    file.sync_all().map_err(err)?;
    Ok(true)
}

#[tauri::command]
fn settings() -> Settings {
    Settings {
        free_preview: cfg!(feature = "free-preview"),
        billing_ready: !cfg!(feature = "free-preview")
            && option_env!("MYSERV_BILLING_URL").is_some()
            && option_env!("MYSERV_ISSUER_PUBLIC_KEY").is_some(),
        mobile: cfg!(any(target_os = "android", target_os = "ios")),
        version: env!("CARGO_PKG_VERSION"),
    }
}
#[tauri::command]
async fn connect(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    ip: String,
    port: u16,
    username: String,
    password: String,
) -> Result<Connected, String> {
    let _lock = state
        .operation
        .try_lock()
        .map_err(|_| "Другая операция уже выполняется")?;
    let credentials = Credentials {
        username,
        password,
        private_key: String::new(),
        passphrase: String::new(),
    };
    let dir = app.path().app_data_dir().map_err(err)?.join("known-hosts");
    let (server, remote) = ssh::Session::connect_remembered(&ip, port, &credentials, &dir)
        .await
        .map_err(err)?;
    let summary = remote
        .script(provision::PREFLIGHT, |_| {})
        .await
        .map_err(err)?;
    let id = Uuid::new_v4().to_string();
    let mut sessions = state.sessions.lock().await;
    sessions.clear();
    sessions.insert(
        id.clone(),
        Arc::new(Mutex::new(Connection {
            server,
            credentials,
            order: None,
        })),
    );
    Ok(Connected {
        session: id,
        summary,
    })
}
#[tauri::command]
async fn disconnect(state: State<'_, AppState>) -> Result<(), String> {
    let _lock = state
        .operation
        .try_lock()
        .map_err(|_| "Дождитесь завершения настройки")?;
    state.sessions.lock().await.clear();
    Ok(())
}
#[tauri::command]
async fn create_payment(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    email: String,
) -> Result<Payment, String> {
    // Store builds must add StoreKit / Play Billing before enabling purchases.
    if cfg!(any(target_os = "ios", target_os = "android")) {
        return Err(
            "Покупки в мобильной сборке ещё не подключены. Оплаченный ключ можно импортировать."
                .into(),
        );
    }
    let s = session(&state, &session_id).await?;
    let mut c = s.lock().await;
    let path = order_path(&app, &c.server.binding().map_err(err)?)?;
    if c.order.is_none() && path.exists() {
        let saved: (Order, String) =
            serde_json::from_slice(&std::fs::read(&path).map_err(err)?).map_err(err)?;
        if saved.0.server_binding != c.server.binding().map_err(err)? {
            return Err("Сохранённый заказ относится к другому серверу".into());
        }
        c.order = Some(saved);
    }
    if c.order.is_none() {
        let mut bytes = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        let access: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let candidate = (
            Order {
                id: Uuid::new_v4(),
                server_binding: c.server.binding().map_err(err)?,
                receipt_email: email,
            },
            access,
        );
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        use std::io::Write;
        let temporary = path.with_extension("tmp");
        let mut file = options.open(&temporary).map_err(err)?;
        file.write_all(&serde_json::to_vec(&candidate).map_err(err)?)
            .map_err(err)?;
        file.sync_all().map_err(err)?;
        drop(file);
        std::fs::rename(temporary, &path).map_err(err)?;
        c.order = Some(candidate);
    }
    let (order, access) = c.order.as_ref().unwrap();
    billing()?.create(order, access).await.map_err(err)
}
#[tauri::command]
async fn check_payment(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    session_id: String,
) -> Result<Payment, String> {
    let s = session(&state, &session_id).await?;
    let mut c = s.lock().await;
    let (order, access) = c.order.as_ref().ok_or("Сначала создайте заказ")?;
    let p = billing()?.check(order.id, access).await.map_err(err)?;
    if p.status == "canceled" {
        std::fs::remove_file(order_path(&app, &order.server_binding)?).map_err(err)?;
        c.order = None;
    }
    Ok(p)
}
#[tauri::command]
async fn deploy(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    options: Options,
    license: String,
) -> Result<Profile, String> {
    let _lock = state
        .operation
        .try_lock()
        .map_err(|_| "Настройка уже выполняется")?;
    let s = session(&state, &session_id).await?;
    let c = s.lock().await;
    let progress = |progress| {
        let _ = app.emit("provision-progress", progress);
    };
    #[cfg(feature = "free-preview")]
    {
        let _ = license;
        provision::deploy_preview(&c.server, &c.credentials, &options, progress)
            .await
            .map_err(err)
    }
    #[cfg(not(feature = "free-preview"))]
    provision::deploy(
        &c.server,
        &c.credentials,
        &options,
        &license,
        issuer()?,
        progress,
    )
    .await
    .map_err(err)
}
#[tauri::command]
fn open_payment(app: tauri::AppHandle, url: String) -> Result<(), String> {
    let parsed = url::Url::parse(&url).map_err(err)?;
    let host = parsed.host_str().unwrap_or("");
    if parsed.scheme() != "https"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || !(host == "yoomoney.ru"
            || host.ends_with(".yoomoney.ru")
            || host == "yookassa.ru"
            || host.ends_with(".yookassa.ru"))
    {
        return Err("Недопустимый адрес оплаты".into());
    }
    app.opener().open_url(url, None::<&str>).map_err(err)
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            settings,
            connect,
            disconnect,
            create_payment,
            check_payment,
            deploy,
            open_payment,
            export_file
        ])
        .run(tauri::generate_context!())
        .expect("Unable to start MyServ");
}
