use crate::config::{AppState, load_config_and_apply_window_settings};
use crate::ui::{initialize_audio_asset_manager, setup_window_event_listeners};
use crate::ui::exit_handler::setup_exit_handlers;
use crate::log_important;
use tauri::{AppHandle, Manager};
use std::time::Duration;
use tokio::time::sleep;

/// 应用设置和初始化
pub async fn setup_application(app_handle: &AppHandle) -> Result<(), String> {
    let state = app_handle.state::<AppState>();

    // 加载配置并应用窗口设置
    if let Err(e) = load_config_and_apply_window_settings(&state, app_handle).await {
        log_important!(warn, "加载配置失败: {}", e);
    }

    // 初始化音频资源管理器
    if let Err(e) = initialize_audio_asset_manager(app_handle) {
        log_important!(warn, "初始化音频资源管理器失败: {}", e);
    }

    // 设置窗口事件监听器
    setup_window_event_listeners(app_handle);

    // 设置退出处理器
    if let Err(e) = setup_exit_handlers(app_handle) {
        log_important!(warn, "设置退出处理器失败: {}", e);
    }

    // 应用设置后显示窗口，避免启动时闪烁到默认位置
    if let Some(window) = app_handle.get_webview_window("main") {
        let (target_width, target_height, pos) = {
            let config = state
                .config
                .lock()
                .map_err(|e| format!("获取配置失败: {}", e))?;
            let window_config = config.ui_config.window_config.clone();
            let (width, height) = if window_config.fixed {
                (window_config.fixed_width, window_config.fixed_height)
            } else {
                (window_config.free_width, window_config.free_height)
            };
            (
                width,
                height,
                (window_config.position_x, window_config.position_y),
            )
        };

        let window = window.clone();
        tauri::async_runtime::spawn(async move {
            let _ = window.hide();
            let _ = window.set_size(tauri::LogicalSize::new(target_width, target_height));
            if let (Some(x), Some(y)) = pos {
                let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
            }
            sleep(Duration::from_millis(16)).await;
            if let Err(e) = window.show() {
                log_important!(warn, "显示主窗口失败: {}", e);
            }
        });
    }

    Ok(())
}
