use tauri::{
    menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, Runtime,
};

pub fn create_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let menu = build_menu(app, crate::is_recording())?;

    TrayIconBuilder::with_id("main-tray")
        .menu(&menu)
        .tooltip("Meetily")
        .icon(app.default_window_icon().unwrap().clone())
        .on_menu_event(|app, event| handle_menu_event(app, event.id.as_ref()))
        .build(app)?;
    Ok(())
}

fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, item_id: &str) {
    match item_id {
        "toggle_recording" => toggle_recording_handler(app),
        "open_window" => focus_main_window(app),
        "settings" => {
            focus_main_window(app);
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.eval("(window.openSettings && window.openSettings())");
            }
        }
        "quit" => app.exit(0),
        _ => {}
    }
}
fn toggle_recording_handler<R: Runtime>(app: &AppHandle<R>) {
    focus_main_window(app);
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        if crate::is_recording() {
            log::info!("Requesting stop recording from tray");
            const STOP_PAYLOAD: &str = r#"
                // This is a custom event that the UI can listen for.
                // It's 'cancelable', so the UI can tell us if it handled the event.
                const event = new CustomEvent('tray-stop-request', { cancelable: true });
                const wasHandledByUI = !window.dispatchEvent(event);

                if (!wasHandledByUI) {
                    console.warn('tray-stop-request was not handled by the UI. Falling back to global handler.');
                    // The global handler is the fallback for when the main UI isn't mounted.
                    window.handleRecordingStop && window.handleRecordingStop();
                }
            "#;
            if let Some(window) = app_clone.get_webview_window("main") {
                let _ = window.eval(STOP_PAYLOAD);
            }
        } else {
            log::info!("Emitting start recording event from tray");
            if let Some(window) = app_clone.get_webview_window("main") {
                let _ = window.eval("sessionStorage.setItem('autoStartRecording', 'true')"); // Set the flag to start recording automatically
                let _ = window.eval("window.location.assign('/')");
            }
        }
    });
}

pub fn update_tray_menu<R: Runtime>(app: &AppHandle<R>) {
    if let Ok(menu) = build_menu(app, crate::is_recording()) {
        if let Some(tray) = app.tray_by_id("main-tray") {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

fn build_menu<R: Runtime>(
    app: &AppHandle<R>,
    is_recording: bool,
) -> tauri::Result<tauri::menu::Menu<R>> {
    let label = if is_recording {
        "Stop Recording"
    } else {
        "Start Recording"
    };
    MenuBuilder::new(app)
        .item(&MenuItemBuilder::with_id("toggle_recording", label).build(app)?)
        .item(&PredefinedMenuItem::separator(app)?)
        .item(&MenuItemBuilder::with_id("open_window", "Open Main Window").build(app)?)
        .item(&MenuItemBuilder::with_id("settings", "Settings").build(app)?)
        .item(&PredefinedMenuItem::separator(app)?)
        .item(&MenuItemBuilder::with_id("quit", "Quit").build(app)?)
        .build()
}

fn focus_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window.eval("window.focus()");
    } else {
        log::warn!("Could not find main window");
    }
}
