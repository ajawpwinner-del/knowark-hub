// 知舟服务台：Tauri 命令层。长任务（安装、下载模型）后台执行，进度经 task-progress 事件推送
mod catalog;
mod docker;
mod env_check;
mod gateway;
mod health;
mod netinfo;
mod ollama;
mod proc;
mod secrets;
mod settings;
mod trial;

use serde::Serialize;
use settings::Settings;
use std::sync::{Arc, RwLock};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;

struct AppState {
    settings: Arc<RwLock<Settings>>,
    shared: gateway::Shared,
    gateway: Mutex<gateway::Gateway>,
}

type R<T> = Result<T, String>;

fn e(err: anyhow::Error) -> String {
    err.to_string()
}

fn def(id: &str) -> R<&'static catalog::ServiceDef> {
    catalog::find(id).ok_or_else(|| format!("未知服务 {id}"))
}

fn snapshot(s: &State<'_, AppState>) -> Settings {
    s.settings.read().unwrap().clone()
}

fn fail(app: &AppHandle, id: &str, msg: String) {
    let _ = app.emit(
        "task-progress",
        docker::TaskProgress { id: id.into(), stage: "error".into(), line: msg.clone(), done: true, error: Some(msg) },
    );
}

#[tauri::command]
async fn check_env(s: State<'_, AppState>) -> R<env_check::EnvReport> {
    Ok(env_check::check(&snapshot(&s).data_dir).await)
}

#[tauri::command]
async fn list_services(s: State<'_, AppState>) -> R<Vec<docker::ServiceState>> {
    let st = snapshot(&s);
    let mut out = Vec::new();
    for d in catalog::services() {
        out.push(docker::state(d, &st).await);
    }
    Ok(out)
}

#[tauri::command]
async fn install_service(app: AppHandle, s: State<'_, AppState>, id: String) -> R<()> {
    let d = def(&id)?;
    let st = snapshot(&s);
    tauri::async_runtime::spawn(async move {
        if let Err(err) = docker::install(&app, d, &st).await {
            fail(&app, &d.id, err.to_string());
        }
    });
    Ok(())
}

#[tauri::command]
async fn service_action(id: String, action: String) -> R<()> {
    match action.as_str() {
        "start" => docker::start(&id).await,
        "stop" => docker::stop(&id).await,
        "restart" => docker::restart(&id).await,
        _ => Err(anyhow::anyhow!("未知操作")),
    }
    .map_err(e)
}

#[tauri::command]
async fn uninstall_service(s: State<'_, AppState>, id: String, remove_data: bool) -> R<()> {
    docker::uninstall(def(&id)?, &snapshot(&s), remove_data).await.map_err(e)
}

#[tauri::command]
async fn service_logs(id: String) -> R<String> {
    Ok(docker::logs(&id, 300).await)
}

#[tauri::command]
async fn probe_service(s: State<'_, AppState>, id: String) -> R<health::Health> {
    let st = snapshot(&s);
    let key = secrets::gateway_key().map_err(e)?;
    Ok(health::probe(&id, st.gateway_port, &key, st.host_port(&id)).await)
}

#[tauri::command]
async fn list_models(s: State<'_, AppState>) -> R<Vec<ollama::LocalModel>> {
    ollama::list(snapshot(&s).host_port("ollama")).await.map_err(e)
}

#[tauri::command]
async fn pull_model(app: AppHandle, s: State<'_, AppState>, name: String) -> R<()> {
    let port = snapshot(&s).host_port("ollama");
    tauri::async_runtime::spawn(async move {
        if let Err(err) = ollama::pull(&app, port, &name).await {
            fail(&app, &format!("model:{name}"), err.to_string());
        }
    });
    Ok(())
}

#[tauri::command]
async fn delete_model(s: State<'_, AppState>, name: String) -> R<()> {
    ollama::delete(snapshot(&s).host_port("ollama"), &name).await.map_err(e)
}

/// 保存服务选项后生效：SearXNG 需重写配置并重启容器；MinerU / OCR 选项由网关实时读取
#[tauri::command]
async fn apply_service_options(s: State<'_, AppState>, id: String) -> R<()> {
    if id == "searxng" {
        docker::write_searxng_config(&snapshot(&s)).map_err(e)?;
        if docker::state(def(&id)?, &snapshot(&s)).await.status == "running" {
            docker::restart(&id).await.map_err(e)?;
        }
    }
    Ok(())
}

#[tauri::command]
async fn try_search(s: State<'_, AppState>, q: String) -> R<Vec<trial::SearchHit>> {
    let key = secrets::gateway_key().map_err(e)?;
    trial::search(snapshot(&s).gateway_port, &key, &q).await.map_err(e)
}

#[tauri::command]
async fn try_parse(s: State<'_, AppState>, path: String, engine: String) -> R<trial::ParseResult> {
    let key = secrets::gateway_key().map_err(e)?;
    trial::parse(snapshot(&s).gateway_port, &key, &path, &engine).await.map_err(e)
}

#[tauri::command]
async fn model_library(kind: String) -> R<Vec<ollama::LibraryModel>> {
    ollama::library(&kind).await.map_err(e)
}

#[tauri::command]
async fn model_dims(s: State<'_, AppState>, name: String) -> R<usize> {
    ollama::embed_dims(snapshot(&s).host_port("ollama"), &name).await.map_err(e)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConnectInfo {
    addresses: Vec<netinfo::LanAddress>,
    gateway_port: u16,
    gateway_error: Option<String>,
    key: String,
    firewall_ready: bool,
}

#[tauri::command]
async fn connect_info(s: State<'_, AppState>) -> R<ConnectInfo> {
    let gw = s.gateway.lock().await;
    Ok(ConnectInfo {
        addresses: netinfo::lan_addresses(),
        gateway_port: gw.port,
        gateway_error: gw.error.clone(),
        key: secrets::gateway_key().map_err(e)?,
        firewall_ready: netinfo::firewall_rule_exists().await,
    })
}

#[tauri::command]
async fn rotate_key(s: State<'_, AppState>) -> R<String> {
    let k = secrets::rotate_key().map_err(e)?;
    *s.shared.key.write().unwrap() = k.clone();
    Ok(k)
}

#[tauri::command]
async fn add_firewall_rule(s: State<'_, AppState>) -> R<()> {
    netinfo::add_firewall_rule(snapshot(&s).gateway_port).await.map_err(e)
}

/// 导出知舟配置文件（路径由系统保存对话框选定）
#[tauri::command]
async fn write_text_file(path: String, text: String) -> R<()> {
    std::fs::write(path, text).map_err(|err| err.to_string())
}

#[tauri::command]
async fn get_settings(s: State<'_, AppState>) -> R<Settings> {
    Ok(snapshot(&s))
}

#[tauri::command]
async fn save_settings(s: State<'_, AppState>, settings: Settings) -> R<()> {
    let old_port = snapshot(&s).gateway_port;
    settings.save().map_err(e)?;
    let new_port = settings.gateway_port;
    *s.settings.write().unwrap() = settings;
    if new_port != old_port {
        let mut gw = s.gateway.lock().await;
        gw.stop();
        *gw = gateway::Gateway::start(s.shared.clone(), new_port).await;
    }
    Ok(())
}

fn show_main(app: &AppHandle) {
    use tauri::Manager;
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    let open = MenuItem::with_id(app, "open", "打开服务台", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出（网关将停止）", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &PredefinedMenuItem::separator(app)?, &quit])?;
    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().cloned().expect("应用图标"))
        .tooltip("知舟服务台")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, e| match e.id.as_ref() {
            "open" => show_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, e| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

pub fn run() {
    let settings = Arc::new(RwLock::new(Settings::load()));
    let key = secrets::gateway_key().unwrap_or_default();
    let shared = gateway::Shared::new(key, settings.clone());

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            use tauri::Manager;
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .manage(AppState { settings: settings.clone(), shared: shared.clone(), gateway: Mutex::new(gateway::Gateway::idle()) })
        // 关闭窗口只缩到托盘：网关要继续为手机上的知舟服务，真正退出走托盘菜单
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(move |app| {
            use tauri::Manager;
            setup_tray(app)?;
            let handle = app.handle().clone();
            let port = settings.read().unwrap().gateway_port;
            tauri::async_runtime::spawn(async move {
                let gw = gateway::Gateway::start(shared, port).await;
                let st = handle.state::<AppState>();
                *st.gateway.lock().await = gw;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            check_env,
            list_services,
            install_service,
            service_action,
            uninstall_service,
            service_logs,
            probe_service,
            list_models,
            pull_model,
            delete_model,
            model_dims,
            model_library,
            apply_service_options,
            try_search,
            try_parse,
            connect_info,
            rotate_key,
            add_firewall_rule,
            write_text_file,
            get_settings,
            save_settings
        ])
        .run(tauri::generate_context!())
        .expect("知舟服务台启动失败");
}
