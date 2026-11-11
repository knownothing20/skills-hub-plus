mod commands;
mod core;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use core::cancel_token::CancelToken;
use core::onboarding::{save_discovery_scan_config, DiscoveryScanConfig};
use core::skill_store::{default_db_path, SkillStore};
use core::tool_adapters::{default_tool_adapters, load_tool_config, save_tool_config, ToolConfig};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};
use tauri_plugin_log::{Target, TargetKind};

/// 最后一次「有人在用界面」的毫秒时间戳。前端节流上报，窗口显示/隐藏时也会刷新。
static LAST_ACTIVITY_MS: AtomicU64 = AtomicU64::new(0);
/// 界面隐藏后闲置这么久就销毁 WebView，把内存还给系统。
const IDLE_DESTROY_MS: u64 = 30 * 60 * 1000;
/// 用户是否明确要求退出。只有它为 true 时才允许进程结束。
static EXITING: AtomicBool = AtomicBool::new(false);
/// 后台巡查间隔：一小时（检查外部源有没有新版本）。
const SCAN_INTERVAL_SECS: u64 = 60 * 60;
/// 副本同步间隔：原件改动后最多这么久就会推送到各工具。
const COPY_SYNC_INTERVAL_SECS: u64 = 5 * 60;

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn touch_activity_now() {
    LAST_ACTIVITY_MS.store(now_ms(), Ordering::Relaxed);
}

/// 前端在鼠标/键盘活动时节流调用，用来推进「闲置」计时。
#[tauri::command]
fn touch_activity() {
    touch_activity_now();
}

/// 显示主窗口；窗口若已被闲置销毁，就重新创建。
fn show_or_create_main_window<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Option<tauri::WebviewWindow<R>> {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        touch_activity_now();
        return Some(window);
    }
    let built =
        tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::App("index.html".into()))
            .title("Skills Hub")
            .inner_size(1200.0, 800.0)
            .min_inner_size(900.0, 600.0)
            .build()
            .ok();
    touch_activity_now();
    built
}

fn open_folder_in_explorer(path: &std::path::Path) {
    if !path.exists() {
        let _ = std::fs::create_dir_all(path);
    }
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer").arg(path).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(path).spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    }
}

fn init_store<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> anyhow::Result<SkillStore> {
    let db_path = default_db_path(app)?;
    let store = SkillStore::new(db_path);
    store.ensure_schema()?;
    ensure_safe_defaults(&store)?;
    let normalized = core::auto_update::normalize_self_sourced_local_skills(&store)?;
    if normalized > 0 {
        log::info!(
            "normalized {} legacy self-sourced local Skill records",
            normalized
        );
    }
    let sanitized = core::auto_update::sanitize_unsafe_git_sources(&store)?;
    if sanitized > 0 {
        log::warn!(
            "cleared {} unsafe legacy Git source references before UI startup",
            sanitized
        );
    }
    Ok(store)
}

fn ensure_safe_defaults(store: &SkillStore) -> anyhow::Result<()> {
    const FULL_MANAGER_MIGRATION_KEY: &str = "safe_full_manager_enabled_v1";
    if store.get_setting(FULL_MANAGER_MIGRATION_KEY)?.is_none() {
        let config = load_tool_config(store)?;
        let all_builtin_keys = default_tool_adapters()
            .into_iter()
            .map(|adapter| adapter.id.as_key().to_string())
            .collect::<std::collections::HashSet<_>>();
        let disabled_keys = config
            .disabled_builtin_tools
            .iter()
            .cloned()
            .collect::<std::collections::HashSet<_>>();
        if config.custom_tools.is_empty() && disabled_keys == all_builtin_keys {
            save_tool_config(store, ToolConfig::default())?;
        }
        store.set_setting(FULL_MANAGER_MIGRATION_KEY, "true")?;
    }

    const DISCOVERY_SCAN_INIT_KEY: &str = "discovery_scan_init_v2";
    if store.get_setting(DISCOVERY_SCAN_INIT_KEY)?.is_none() {
        save_discovery_scan_config(
            store,
            DiscoveryScanConfig {
                disabled_source_keys: Vec::new(),
            },
        )?;
        store.set_setting(DISCOVERY_SCAN_INIT_KEY, "true")?;
    }

    if store
        .get_setting(core::cache_cleanup::GIT_CACHE_CLEANUP_DAYS_KEY)?
        .is_none()
    {
        store.set_setting(core::cache_cleanup::GIT_CACHE_CLEANUP_DAYS_KEY, "0")?;
    }
    Ok(())
}

fn refresh_enabled_auto_update_task(store: &SkillStore) -> anyhow::Result<()> {
    let config = core::auto_update::get_auto_update_config(store)?;
    if !config.enabled {
        return Ok(());
    }

    let scheduler = core::system_scheduler::current_scheduler_config(config.schedule)?;
    if core::system_scheduler::ensure_installed_auto_update_task(&scheduler)? {
        log::info!("repaired automatic update task registration");
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "windows")]
    {
        let existing = std::env::var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS").unwrap_or_default();
        if !existing.contains("--disable-features=CalculateNativeWinOcclusion") {
            let extra = "--disable-features=CalculateNativeWinOcclusion --disable-gpu-compositing";
            let args = if existing.is_empty() {
                extra.to_string()
            } else {
                format!("{existing} {extra}")
            };
            std::env::set_var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", args);
        }
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            app.handle().plugin(
                tauri_plugin_log::Builder::default()
                    .level(log::LevelFilter::Info)
                    .targets([
                        Target::new(TargetKind::LogDir { file_name: None }),
                        #[cfg(desktop)]
                        Target::new(TargetKind::Stdout),
                    ])
                    .build(),
            )?;

            let store = init_store(app.handle()).map_err(tauri::Error::from)?;

            let is_background_update = std::env::args()
                .collect::<Vec<_>>()
                .windows(2)
                .any(|pair| pair[0] == "--background-task" && pair[1] == "update-skills");
            let force_background_update = std::env::args().any(|arg| arg == "--force");

            if is_background_update {
                #[cfg(target_os = "macos")]
                {
                    app.set_activation_policy(tauri::ActivationPolicy::Accessory);
                }
                let run_result = if force_background_update {
                    core::auto_update::run_auto_update_now(app.handle(), &store).map(Some)
                } else {
                    core::auto_update::run_due_auto_update(app.handle(), &store)
                };
                match run_result {
                    Ok(Some(result)) => {
                        log::info!(
                            "auto update finished: checked={}, updated={}, skipped={}, failed={}",
                            result.checked,
                            result.updated,
                            result.skipped,
                            result.failed
                        );
                        app.handle().exit(if result.failed == 0 { 0 } else { 2 });
                    }
                    Ok(None) => app.handle().exit(0),
                    Err(err) => {
                        eprintln!("auto update failed: {err:#}");
                        app.handle().exit(1);
                    }
                }
                return Ok(());
            }

            // Keep an enabled background task bound to the current installed
            // executable after the app is renamed or moved. A registration
            // repair must never prevent the foreground manager from opening.
            if let Err(err) = refresh_enabled_auto_update_task(&store) {
                log::warn!("failed to refresh automatic update task: {err:#}");
            }

            app.manage(store.clone());
            app.manage(Arc::new(CancelToken::new()));

            if let Err(err) = core::central_repo::migrate_legacy_central_if_needed(app.handle(), &store) {
                log::warn!("legacy central migration warning: {err:#}");
            }

            let handle_clone = app.handle().clone();
            let store_clone = store.clone();
            tauri::async_runtime::spawn_blocking(move || {
                match core::installer::adopt_existing_central_skills(&handle_clone, &store_clone) {
                    Ok(adopted) => {
                        log::info!(
                            "safe central adoption: adopted={}, existing={}, invalid={}, symlink={}, missing_skill_md={}, other={}",
                            adopted.adopted,
                            adopted.already_registered,
                            adopted.skipped_invalid_name,
                            adopted.skipped_symlink,
                            adopted.skipped_missing_skill_md,
                            adopted.skipped_other
                        );
                    }
                    Err(err) => {
                        log::warn!("background central adoption warning: {err:#}");
                    }
                }
                core::installer::backfill_skill_descriptions(&store_clone);

                // 如果启用了自动备份，启动时异步静默更新备份
                let config = core::backup::get_backup_config(&store_clone);
                if config.enabled {
                    log::info!("running initial silent auto-backup to {:?}", config.backup_dir);
                    let _ = core::backup::perform_backup(&handle_clone, &store_clone, None);
                }
            });

            // 建立系统托盘菜单与托盘图标
            let show_item = MenuItem::with_id(app, "show", "显示 Skills Hub", true, None::<&str>)?;
            let sep1 = PredefinedMenuItem::separator(app)?;
            let central_skills_path = core::central_repo::app_install_skills_dir();
            let central_title = format!("📂 打开技能中心母库 ({})", central_skills_path.to_string_lossy());
            let central_item = MenuItem::with_id(
                app,
                "open_central",
                &central_title,
                true,
                None::<&str>,
            )?;
            let sep2 = PredefinedMenuItem::separator(app)?;
            let sync_item = MenuItem::with_id(
                app,
                "sync_now",
                "🔄 立即检查并同步技能",
                true,
                None::<&str>,
            )?;
            let sep3 = PredefinedMenuItem::separator(app)?;
            let quit_item = MenuItem::with_id(app, "quit", "❌ 退出 Skills Hub", true, None::<&str>)?;

            let tray_menu = Menu::with_items(
                app,
                &[
                    &show_item,
                    &sep1,
                    &central_item,
                    &sep2,
                    &sync_item,
                    &sep3,
                    &quit_item,
                ],
            )?;

            if let Some(app_icon) = app.default_window_icon() {
                let _tray = TrayIconBuilder::new()
                    .icon(app_icon.clone())
                    .tooltip("Skills Hub - Agent 技能管家")
                    .menu(&tray_menu)
                    .show_menu_on_left_click(false)
                    .on_menu_event(move |app_handle, event| match event.id.as_ref() {
                        "show" => {
                            let _ = show_or_create_main_window(app_handle);
                        }
                        "open_central" => {
                            let central_skills_path = core::central_repo::app_install_skills_dir();
                            open_folder_in_explorer(&central_skills_path);
                        }
                        "sync_now" => {
                            let handle = app_handle.clone();
                            let store = app_handle.state::<SkillStore>().inner().clone();
                            tauri::async_runtime::spawn(async move {
                                log::info!("manual sync triggered from system tray");
                                // 先把落后副本刷新成原件内容，再检查外部有没有新版本
                                let central_root = core::central_repo::app_install_skills_dir();
                                match core::stale_copies::startup_stale_refresh(&store, &central_root) {
                                    Ok((refreshed, remaining)) => log::info!(
                                        "manual copy refresh: refreshed={refreshed} remaining={remaining}"
                                    ),
                                    Err(err) => log::warn!("manual copy refresh failed: {err:#}"),
                                }
                                let _ = core::auto_update::run_auto_update_now(&handle, &store);
                            });
                        }
                        "quit" => {
                            EXITING.store(true, Ordering::Relaxed);
                            app_handle.exit(0);
                        }
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } = event
                        {
                            let app = tray.app_handle();
                            let visible = app
                                .get_webview_window("main")
                                .map(|w| w.is_visible().unwrap_or(false))
                                .unwrap_or(false);
                            if visible {
                                if let Some(window) = app.get_webview_window("main") {
                                    let _ = window.hide();
                                }
                                // 隐藏的这一刻作为闲置计时起点
                                touch_activity_now();
                            } else {
                                let _ = show_or_create_main_window(app);
                            }
                        }
                    })
                    .build(app)?;
            }

            // 点窗口的 × 只隐藏到托盘，不退出；退出走托盘菜单
            if let Some(window) = app.get_webview_window("main") {
                let window_for_event = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window_for_event.hide();
                        touch_activity_now();
                    }
                });
            }

            // 检查启动参数是否包含最小化启动
            let start_minimized = std::env::args().any(|arg| arg == "--minimized" || arg == "-m");
            if start_minimized {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
            touch_activity_now();

            // 每 5 分钟把原件的新内容推给落后副本（各工具、各账户）
            {
                let handle = app.handle().clone();
                let store = app.state::<SkillStore>().inner().clone();
                std::thread::spawn(move || loop {
                    std::thread::sleep(std::time::Duration::from_secs(COPY_SYNC_INTERVAL_SECS));
                    let central_root = core::central_repo::app_install_skills_dir();
                    match core::stale_copies::startup_stale_refresh(&store, &central_root) {
                        Ok((refreshed, remaining)) => {
                            if refreshed > 0 || remaining > 0 {
                                log::info!(
                                    "periodic copy refresh: refreshed={refreshed} remaining={remaining}"
                                );
                            }
                        }
                        Err(err) => log::warn!("periodic copy refresh failed: {err:#}"),
                    }
                    // 副本更新后让界面重新拉一次数据
                    let _ = handle.emit("skills-updated", ());
                });
            }

            // 每小时静默巡查一次：比对各目标、需要时刷新副本
            {
                let handle = app.handle().clone();
                let store = app.state::<SkillStore>().inner().clone();
                std::thread::spawn(move || loop {
                    std::thread::sleep(std::time::Duration::from_secs(SCAN_INTERVAL_SECS));
                    log::info!("hourly scan: start");
                    match core::auto_update::run_auto_update_now(&handle, &store) {
                        Ok(_) => log::info!("hourly scan: done"),
                        Err(err) => log::warn!("hourly scan failed: {err:#}"),
                    }
                });
            }

            // 界面隐藏后闲置 30 分钟：销毁 WebView，把内存还给系统。
            // 只销毁隐藏的窗口——可见时突然消失太突兀。
            {
                let handle = app.handle().clone();
                // 允许用环境变量临时改阈值，便于验证销毁逻辑（正式使用不设即可）
                let idle_limit_ms = std::env::var("SKILLS_HUB_IDLE_DESTROY_SECS")
                    .ok()
                    .and_then(|value| value.parse::<u64>().ok())
                    .map(|secs| secs.saturating_mul(1000))
                    .unwrap_or(IDLE_DESTROY_MS);
                log::info!("idle destroy threshold: {idle_limit_ms} ms");
                std::thread::spawn(move || loop {
                    std::thread::sleep(std::time::Duration::from_secs(60));
                    let Some(window) = handle.get_webview_window("main") else {
                        continue;
                    };
                    if window.is_visible().unwrap_or(true) {
                        continue;
                    }
                    let idle = now_ms().saturating_sub(LAST_ACTIVITY_MS.load(Ordering::Relaxed));
                    if idle >= idle_limit_ms {
                        log::info!("window hidden and idle {idle}ms: destroying webview to free memory");
                        let _ = window.destroy();
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            touch_activity,
            commands::list_central_library,
            commands::list_library_files,
            commands::read_library_file,
            commands::get_central_repo_path,
            commands::get_recent_projects,
            commands::save_recent_project,
            commands::get_tool_status,
            commands::get_tool_config,
            commands::set_tool_config,
            commands::get_git_cache_ttl_secs,
            commands::set_git_cache_ttl_secs,
            commands::clear_git_cache_now,
            commands::get_auto_update_config,
            commands::set_auto_update_config,
            commands::run_auto_update_now,
            commands::trigger_auto_update_task_now_cmd,
            commands::get_onboarding_plan,
            commands::get_discovery_scan_settings,
            commands::set_discovery_scan_config,
            commands::get_ignored_tools,
            commands::set_ignored_tools,
            commands::get_tool_sync_modes,
            commands::set_tool_sync_mode,
            commands::get_shared_skill_dirs,
            commands::get_tool_root_instances,
            commands::sync_skill_to_tool_instances,
            commands::get_stale_mirrors,
            commands::get_stale_copy_status,
            commands::refresh_stale_copy_cmd,
            commands::run_startup_stale_refresh,
            commands::get_auto_refresh_stale_copies,
            commands::set_auto_refresh_stale_copies_cmd,
            commands::get_tool_skill_whitelist,
            commands::set_tool_skill_whitelist,
            commands::install_local,
            commands::list_local_skills_cmd,
            commands::install_local_selection,
            commands::install_git,
            commands::list_git_skills_cmd,
            commands::install_git_selection,
            commands::sync_skill_dir,
            commands::sync_skill_to_tool,
            commands::unsync_skill_from_tool,
            commands::set_skill_enabled,
            commands::enable_skill_and_restore_targets,
            commands::update_managed_skill,
            commands::search_github,
            commands::get_github_proxy_config,
            commands::set_github_proxy_config,
            commands::import_existing_skill,
            commands::get_managed_skills,
            commands::get_tags,
            commands::create_tag,
            commands::rename_tag,
            commands::delete_tag,
            commands::get_skill_tags,
            commands::set_skill_tags,
            commands::get_untagged_skill_ids,
            commands::delete_managed_skill,
            commands::ignore_managed_skill,
            commands::ignore_skill,
            commands::get_ignored_skills,
            commands::unignore_skill,
            commands::get_featured_skills,
            commands::search_skills_online,
            commands::list_skill_files,
            commands::read_skill_file,
            commands::get_skill_ignore_config,
            commands::save_skill_ignore_config,
            commands::reconcile_all_skill_targets,
            commands::cancel_current_operation,
            commands::get_skill_target_comparisons,
            commands::promote_target_to_central,
            commands::promote_target_to_owner,
            commands::get_backup_config,
            commands::save_backup_config,
            commands::create_backup_now,
            commands::restore_backup_now,
            commands::get_default_sync_mode,
            commands::set_default_sync_mode,
            commands::get_library_health,
            commands::export_skills_lock_cmd,
            commands::import_skills_lock_cmd
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|_app, event| {
            // 闲置回收销毁了主窗口，不能顺带把进程也结束掉：
            // 托盘还得活着，否则「30 分钟省内存」变成「30 分钟自己退出」。
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                if !EXITING.load(Ordering::Relaxed) {
                    api.prevent_exit();
                }
            }
        });
}
