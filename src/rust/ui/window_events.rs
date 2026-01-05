use crate::config::{AppState, save_config};
use crate::constants::{validation, window as window_constants};
use crate::log_important;
use tauri::{AppHandle, Manager, WindowEvent};

/// 设置窗口事件监听器
pub fn setup_window_event_listeners(app_handle: &AppHandle) {
    if let Some(window) = app_handle.get_webview_window("main") {
        let app_handle_clone = app_handle.clone();
        
        window.on_window_event(move |event| {
            match event {
                WindowEvent::CloseRequested { api, .. } => {
                // 阻止默认的关闭行为
                api.prevent_close();
                
                let app_handle = app_handle_clone.clone();
                
                // 异步处理退出请求
                tauri::async_runtime::spawn(async move {
                    let state = app_handle.state::<AppState>();

                    // log_important!(info, "🖱️ 窗口关闭按钮被点击");

                    // 窗口关闭按钮点击应该直接退出，不需要双重确认
                    match crate::ui::exit::handle_system_exit_request(
                        state,
                        &app_handle,
                        true, // 手动点击关闭按钮
                    ).await {
                        Ok(exited) => {
                            if !exited {
                                log_important!(info, "退出被阻止，等待二次确认");
                            } else {
                                // log_important!(info, "应用已退出");
                            }
                        }
                        Err(e) => {
                            log_important!(error, "处理退出请求失败: {}", e);
                        }
                    }
                });
                }
                WindowEvent::Moved(position) => {
                    let x = position.x;
                    let y = position.y;
                    let app_handle = app_handle_clone.clone();
                    tauri::async_runtime::spawn(async move {
                        let state = app_handle.state::<AppState>();
                        if validation::is_valid_window_position(x, y) {
                            {
                                let mut config = match state.config.lock() {
                                    Ok(guard) => guard,
                                    Err(_) => return,
                                };
                                config.ui_config.window_config.position_x = Some(x);
                                config.ui_config.window_config.position_y = Some(y);
                            }
                            let _ = save_config(&state, &app_handle).await;
                        }
                    });
                }
                WindowEvent::Resized(size) => {
                    let width = size.width;
                    let height = size.height;
                    let app_handle = app_handle_clone.clone();
                    tauri::async_runtime::spawn(async move {
                        let state = app_handle.state::<AppState>();
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let scale_factor = window.scale_factor().unwrap_or(1.0);
                            let logical_width = width as f64 / scale_factor;
                            let logical_height = height as f64 / scale_factor;
                            let (clamped_width, clamped_height) =
                                window_constants::clamp_window_size(logical_width, logical_height);

                            {
                                let mut config = match state.config.lock() {
                                    Ok(guard) => guard,
                                    Err(_) => return,
                                };
                                config
                                    .ui_config
                                    .window_config
                                    .update_current_size(clamped_width, clamped_height);
                            }

                            let _ = save_config(&state, &app_handle).await;
                        }
                    });
                }
                _ => {}
            }
        });
    }
}
