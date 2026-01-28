use crate::proxy::monitor::{ProxyMonitor, ProxyRequestLog, ProxyStats};
use crate::proxy::{ProxyConfig, TokenManager};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;
use tokio::time::Duration;

/// 反代服务状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyStatus {
    pub running: bool,
    pub port: u16,
    pub base_url: String,
    pub active_accounts: usize,
}

/// 反代服务全局状态
pub struct ProxyServiceState {
    pub instance: Arc<RwLock<Option<ProxyServiceInstance>>>,
    pub monitor: Arc<RwLock<Option<Arc<ProxyMonitor>>>>,
}

/// 反代服务实例
pub struct ProxyServiceInstance {
    pub config: ProxyConfig,
    pub token_manager: Arc<TokenManager>,
    pub axum_server: crate::proxy::AxumServer,
    pub server_handle: tokio::task::JoinHandle<()>,
}

impl ProxyServiceState {
    pub fn new() -> Self {
        Self {
            instance: Arc::new(RwLock::new(None)),
            monitor: Arc::new(RwLock::new(None)),
        }
    }
}

/// 启动反代服务
#[tauri::command]
pub async fn start_proxy_service(
    config: ProxyConfig,
    state: State<'_, ProxyServiceState>,
    app_handle: tauri::AppHandle,
) -> Result<ProxyStatus, String> {
    let mut instance_lock = state.instance.write().await;

    // 防止重复启动
    if instance_lock.is_some() {
        return Err("服务已在运行中".to_string());
    }

    // Ensure monitor exists
    {
        let mut monitor_lock = state.monitor.write().await;
        if monitor_lock.is_none() {
            *monitor_lock = Some(Arc::new(ProxyMonitor::new(1000, Some(app_handle.clone()))));
        }
        // Sync enabled state from config
        if let Some(monitor) = monitor_lock.as_ref() {
            monitor.set_enabled(config.enable_logging);
        }
    }

    let monitor = state.monitor.read().await.as_ref().unwrap().clone();

    // 2. 初始化 Token 管理器
    let app_data_dir = crate::modules::account::get_data_dir()?;
    // Ensure accounts dir exists even if the user will only use non-Google providers (e.g. z.ai).
    let _ = crate::modules::account::get_accounts_dir()?;
    let accounts_dir = app_data_dir.clone();

    let token_manager = Arc::new(TokenManager::new(accounts_dir));
    token_manager.start_auto_cleanup(); // 启动限流记录自动清理后台任务
                                        // 同步 UI 传递的调度配置
    token_manager
        .update_sticky_config(config.scheduling.clone())
        .await;

    // 🆕 [FIX #820] 恢复固定账号模式设置
    if let Some(ref account_id) = config.preferred_account_id {
        token_manager
            .set_preferred_account(Some(account_id.clone()))
            .await;
        tracing::info!("🔒 [FIX #820] Fixed account mode restored: {}", account_id);
    }

    // 檢查並啟動管理服務器（如果尚未運行）
    // ensure_admin_server(config.clone(), state, integration.clone(), cloudflared_state.clone()).await?;

    // 3. 加載賬號
    let active_accounts = token_manager
        .load_accounts()
        .await
        .map_err(|e| format!("加载账号失败: {}", e))?;

    if active_accounts == 0 {
        let zai_enabled = config.zai.enabled
            && !matches!(config.zai.dispatch_mode, crate::proxy::ZaiDispatchMode::Off);
        if !zai_enabled {
            return Err("没有可用账号，请先添加账号".to_string());
        }
    }

    // 启动 Axum 服务器
    let (axum_server, server_handle) = match crate::proxy::AxumServer::start(
        config.get_bind_address().to_string(),
        config.port,
        token_manager.clone(),
        config.custom_mapping.clone(),
        config.request_timeout,
        config.upstream_proxy.clone(),
        crate::proxy::ProxySecurityConfig::from_proxy_config(&config),
        config.zai.clone(),
        monitor.clone(),
        config.experimental.clone(),
    )
    .await
    {
        Ok((server, handle)) => (server, handle),
        Err(e) => return Err(format!("启动 Axum 服务器失败: {}", e)),
    };

    // 创建服务实例
    let instance = ProxyServiceInstance {
        config: config.clone(),
        token_manager: token_manager.clone(), // Clone for ProxyServiceInstance
        axum_server,
        server_handle,
    };

    *instance_lock = Some(instance);

    // 保存配置到全局 AppConfig
    let mut app_config = crate::modules::config::load_app_config().map_err(|e| e)?;
    app_config.proxy = config.clone();
    crate::modules::config::save_app_config(&app_config).map_err(|e| e)?;

    Ok(ProxyStatus {
        running: true,
        port: config.port,
        base_url: format!("http://127.0.0.1:{}", config.port),
        active_accounts,
    })
}

/// 停止反代服务
#[tauri::command]
pub async fn stop_proxy_service(state: State<'_, ProxyServiceState>) -> Result<(), String> {
    let mut instance_lock = state.instance.write().await;

    if instance_lock.is_none() {
        return Err("服务未运行".to_string());
    }

    // 停止 Axum 服务器
    if let Some(instance) = instance_lock.take() {
        instance.axum_server.stop();
        // 等待服务器任务完成
        instance.server_handle.await.ok();
    }

    Ok(())
}

/// 获取反代服务状态
#[tauri::command]
pub async fn get_proxy_status(state: State<'_, ProxyServiceState>) -> Result<ProxyStatus, String> {
    let instance_lock = state.instance.read().await;

    match instance_lock.as_ref() {
        Some(instance) => Ok(ProxyStatus {
            running: true,
            port: instance.config.port,
            base_url: format!("http://127.0.0.1:{}", instance.config.port),
            active_accounts: instance.token_manager.len(),
        }),
        None => Ok(ProxyStatus {
            running: false,
            port: 0,
            base_url: String::new(),
            active_accounts: 0,
        }),
    }
}

/// 获取反代服务统计
#[tauri::command]
pub async fn get_proxy_stats(state: State<'_, ProxyServiceState>) -> Result<ProxyStats, String> {
    let monitor_lock = state.monitor.read().await;
    if let Some(monitor) = monitor_lock.as_ref() {
        Ok(monitor.get_stats().await)
    } else {
        Ok(ProxyStats::default())
    }
}

/// 获取反代请求日志
#[tauri::command]
pub async fn get_proxy_logs(
    state: State<'_, ProxyServiceState>,
    limit: Option<usize>,
) -> Result<Vec<ProxyRequestLog>, String> {
    let monitor_lock = state.monitor.read().await;
    if let Some(monitor) = monitor_lock.as_ref() {
        Ok(monitor.get_logs(limit.unwrap_or(100)).await)
    } else {
        Ok(Vec::new())
    }
}

/// 设置监控开启状态
#[tauri::command]
pub async fn set_proxy_monitor_enabled(
    state: State<'_, ProxyServiceState>,
    enabled: bool,
) -> Result<(), String> {
    let monitor_lock = state.monitor.read().await;
    if let Some(monitor) = monitor_lock.as_ref() {
        monitor.set_enabled(enabled);
    }
    Ok(())
}

/// 清除反代请求日志
#[tauri::command]
pub async fn clear_proxy_logs(state: State<'_, ProxyServiceState>) -> Result<(), String> {
    let monitor_lock = state.monitor.read().await;
    if let Some(monitor) = monitor_lock.as_ref() {
        monitor.clear().await;
    }
    Ok(())
}

/// 获取反代请求日志 (分页)
#[tauri::command]
pub async fn get_proxy_logs_paginated(
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<ProxyRequestLog>, String> {
    crate::modules::proxy_db::get_logs_summary(limit.unwrap_or(20), offset.unwrap_or(0))
}

/// 获取单条日志的完整详情
#[tauri::command]
pub async fn get_proxy_log_detail(log_id: String) -> Result<ProxyRequestLog, String> {
    crate::modules::proxy_db::get_log_detail(&log_id)
}

/// 获取日志总数
#[tauri::command]
pub async fn get_proxy_logs_count() -> Result<u64, String> {
    crate::modules::proxy_db::get_logs_count()
}

/// 导出所有日志到指定文件
#[tauri::command]
pub async fn export_proxy_logs(file_path: String) -> Result<usize, String> {
    let logs = crate::modules::proxy_db::get_all_logs_for_export()?;
    let count = logs.len();

    let json = serde_json::to_string_pretty(&logs)
        .map_err(|e| format!("Failed to serialize logs: {}", e))?;

    std::fs::write(&file_path, json).map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(count)
}

/// 导出指定的日志JSON到文件
#[tauri::command]
pub async fn export_proxy_logs_json(file_path: String, json_data: String) -> Result<usize, String> {
    // Parse to count items
    let logs: Vec<serde_json::Value> =
        serde_json::from_str(&json_data).map_err(|e| format!("Failed to parse JSON: {}", e))?;
    let count = logs.len();

    // Pretty print
    let pretty_json =
        serde_json::to_string_pretty(&logs).map_err(|e| format!("Failed to serialize: {}", e))?;

    std::fs::write(&file_path, pretty_json).map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(count)
}

/// 获取带搜索条件的日志数量
#[tauri::command]
pub async fn get_proxy_logs_count_filtered(
    filter: String,
    errors_only: bool,
) -> Result<u64, String> {
    crate::modules::proxy_db::get_logs_count_filtered(&filter, errors_only)
}

/// 获取带搜索条件的分页日志
#[tauri::command]
pub async fn get_proxy_logs_filtered(
    filter: String,
    errors_only: bool,
    limit: usize,
    offset: usize,
) -> Result<Vec<crate::proxy::monitor::ProxyRequestLog>, String> {
    crate::modules::proxy_db::get_logs_filtered(&filter, errors_only, limit, offset)
}

/// 生成 API Key
#[tauri::command]
pub fn generate_api_key() -> String {
    format!("sk-{}", uuid::Uuid::new_v4().simple())
}

/// 重新加载账号（当主应用添加/删除账号时调用）
#[tauri::command]
pub async fn reload_proxy_accounts(state: State<'_, ProxyServiceState>) -> Result<usize, String> {
    let instance_lock = state.instance.read().await;

    if let Some(instance) = instance_lock.as_ref() {
        // [FIX #820] Clear stale session bindings before reloading accounts
        // This ensures that after switching accounts in the UI, API requests
        // won't be routed to the previously bound (wrong) account
        instance.token_manager.clear_all_sessions();

        // 重新加载账号
        let count = instance
            .token_manager
            .load_accounts()
            .await
            .map_err(|e| format!("重新加载账号失败: {}", e))?;
        Ok(count)
    } else {
        Err("服务未运行".to_string())
    }
}

/// 更新模型映射表 (热更新)
#[tauri::command]
pub async fn update_model_mapping(
    config: ProxyConfig,
    state: State<'_, ProxyServiceState>,
) -> Result<(), String> {
    let instance_lock = state.instance.read().await;

    // 1. 如果服务正在运行，立即更新内存中的映射 (这里目前只更新了 anthropic_mapping 的 RwLock,
    // 后续可以根据需要让 resolve_model_route 直接读取全量 config)
    if let Some(instance) = instance_lock.as_ref() {
        instance.axum_server.update_mapping(&config).await;
        tracing::debug!("后端服务已接收全量模型映射配置");
    }

    // 2. 无论是否运行，都保存到全局配置持久化
    let mut app_config = crate::modules::config::load_app_config().map_err(|e| e)?;
    app_config.proxy.custom_mapping = config.custom_mapping;
    crate::modules::config::save_app_config(&app_config).map_err(|e| e)?;

    Ok(())
}

fn join_base_url(base: &str, path: &str) -> String {
    let base = base.trim_end_matches('/');
    let path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{}", path)
    };
    format!("{}{}", base, path)
}

fn extract_model_ids(value: &serde_json::Value) -> Vec<String> {
    let mut out = Vec::new();

    fn push_from_item(out: &mut Vec<String>, item: &serde_json::Value) {
        match item {
            serde_json::Value::String(s) => out.push(s.to_string()),
            serde_json::Value::Object(map) => {
                if let Some(id) = map.get("id").and_then(|v| v.as_str()) {
                    out.push(id.to_string());
                } else if let Some(name) = map.get("name").and_then(|v| v.as_str()) {
                    out.push(name.to_string());
                }
            }
            _ => {}
        }
    }

    match value {
        serde_json::Value::Array(arr) => {
            for item in arr {
                push_from_item(&mut out, item);
            }
        }
        serde_json::Value::Object(map) => {
            if let Some(data) = map.get("data") {
                if let serde_json::Value::Array(arr) = data {
                    for item in arr {
                        push_from_item(&mut out, item);
                    }
                }
            }
            if let Some(models) = map.get("models") {
                match models {
                    serde_json::Value::Array(arr) => {
                        for item in arr {
                            push_from_item(&mut out, item);
                        }
                    }
                    other => push_from_item(&mut out, other),
                }
            }
        }
        _ => {}
    }

    out
}

/// Fetch available models from the configured z.ai Anthropic-compatible API (`/v1/models`).
#[tauri::command]
pub async fn fetch_zai_models(
    zai: crate::proxy::ZaiConfig,
    upstream_proxy: crate::proxy::config::UpstreamProxyConfig,
    request_timeout: u64,
) -> Result<Vec<String>, String> {
    if zai.base_url.trim().is_empty() {
        return Err("z.ai base_url is empty".to_string());
    }
    if zai.api_key.trim().is_empty() {
        return Err("z.ai api_key is not set".to_string());
    }

    let url = join_base_url(&zai.base_url, "/v1/models");

    let mut builder =
        reqwest::Client::builder().timeout(Duration::from_secs(request_timeout.max(5)));
    if upstream_proxy.enabled && !upstream_proxy.url.is_empty() {
        let proxy = reqwest::Proxy::all(&upstream_proxy.url)
            .map_err(|e| format!("Invalid upstream proxy url: {}", e))?;
        builder = builder.proxy(proxy);
    }
    let client = builder
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", zai.api_key))
        .header("x-api-key", zai.api_key)
        .header("anthropic-version", "2023-06-01")
        .header("accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("Upstream request failed: {}", e))?;

    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    if !status.is_success() {
        let preview = if text.len() > 4000 {
            &text[..4000]
        } else {
            &text
        };
        return Err(format!("Upstream returned {}: {}", status, preview));
    }

    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("Invalid JSON response: {}", e))?;
    let mut models = extract_model_ids(&json);
    models.retain(|s| !s.trim().is_empty());
    models.sort();
    models.dedup();
    Ok(models)
}

/// 获取当前调度配置
#[tauri::command]
pub async fn get_proxy_scheduling_config(
    state: State<'_, ProxyServiceState>,
) -> Result<crate::proxy::sticky_config::StickySessionConfig, String> {
    let instance_lock = state.instance.read().await;
    if let Some(instance) = instance_lock.as_ref() {
        Ok(instance.token_manager.get_sticky_config().await)
    } else {
        Ok(crate::proxy::sticky_config::StickySessionConfig::default())
    }
}

/// 更新调度配置
#[tauri::command]
pub async fn update_proxy_scheduling_config(
    state: State<'_, ProxyServiceState>,
    config: crate::proxy::sticky_config::StickySessionConfig,
) -> Result<(), String> {
    let instance_lock = state.instance.read().await;
    if let Some(instance) = instance_lock.as_ref() {
        instance.token_manager.update_sticky_config(config).await;
        Ok(())
    } else {
        Err("服务未运行，无法更新实时配置".to_string())
    }
}

/// 清除所有会话粘性绑定
#[tauri::command]
pub async fn clear_proxy_session_bindings(
    state: State<'_, ProxyServiceState>,
) -> Result<(), String> {
    let instance_lock = state.instance.read().await;
    if let Some(instance) = instance_lock.as_ref() {
        instance.token_manager.clear_all_sessions();
        Ok(())
    } else {
        Err("服务未运行".to_string())
    }
}

// ===== [FIX #820] 固定账号模式命令 =====

/// 设置优先使用的账号（固定账号模式）
/// 传入 account_id 启用固定模式，传入 null/空字符串恢复轮询模式
#[tauri::command]
pub async fn set_preferred_account(
    state: State<'_, ProxyServiceState>,
    account_id: Option<String>,
) -> Result<(), String> {
    let instance_lock = state.instance.read().await;
    if let Some(instance) = instance_lock.as_ref() {
        // 过滤空字符串为 None
        let cleaned_id = account_id.filter(|s| !s.trim().is_empty());

        // 1. 更新内存状态
        instance
            .token_manager
            .set_preferred_account(cleaned_id.clone())
            .await;

        // 2. 持久化到配置文件 (修复 Issue #820 自动关闭问题)
        let mut app_config = crate::modules::config::load_app_config()
            .map_err(|e| format!("加载配置失败: {}", e))?;
        app_config.proxy.preferred_account_id = cleaned_id.clone();
        crate::modules::config::save_app_config(&app_config)
            .map_err(|e| format!("保存配置失败: {}", e))?;

        if let Some(ref id) = cleaned_id {
            tracing::info!(
                "🔒 [FIX #820] Fixed account mode enabled and persisted: {}",
                id
            );
        } else {
            tracing::info!("🔄 [FIX #820] Round-robin mode enabled and persisted");
        }

        Ok(())
    } else {
        Err("服务未运行".to_string())
    }
}

/// 获取当前优先使用的账号ID
#[tauri::command]
pub async fn get_preferred_account(
    state: State<'_, ProxyServiceState>,
) -> Result<Option<String>, String> {
    let instance_lock = state.instance.read().await;
    if let Some(instance) = instance_lock.as_ref() {
        Ok(instance.token_manager.get_preferred_account().await)
    } else {
        Ok(None)
    }
}
