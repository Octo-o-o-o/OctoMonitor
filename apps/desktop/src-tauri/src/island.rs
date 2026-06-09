use objc2_app_kit::NSScreen;
use objc2_foundation::{
    MainThreadMarker as ObjcMainThreadMarker, NSPoint as ObjcNSPoint, NSRect as ObjcNSRect,
};
use tauri::{window::Color, AppHandle, LogicalPosition, LogicalSize, Manager, Position, Size, WebviewUrl};
use tauri_nspanel::{
    tauri_panel, CollectionBehavior, ManagerExt, Panel, PanelBuilder, PanelLevel, StyleMask,
};

pub const ISLAND_WINDOW_LABEL: &str = "island";

const ISLAND_WIDTH: f64 = 408.0;
const ISLAND_HEIGHT: f64 = 336.0;
const TOP_MARGIN: f64 = 10.0;

tauri_panel! {
    panel!(IslandPanel {
        config: {
            can_become_key_window: false,
            can_become_main_window: false,
            is_floating_panel: true
        }
    })
}

fn initial_panel_position(app: &AppHandle) -> LogicalPosition<f64> {
    let Some(monitor) = app.primary_monitor().ok().flatten() else {
        return LogicalPosition::new(0.0, TOP_MARGIN);
    };
    let scale = monitor.scale_factor();
    let position = monitor.position();
    let size = monitor.size();
    let monitor_x = position.x as f64 / scale;
    let monitor_y = position.y as f64 / scale;
    let monitor_width = size.width as f64 / scale;

    LogicalPosition::new(
        monitor_x + ((monitor_width - ISLAND_WIDTH) / 2.0).max(0.0),
        monitor_y + TOP_MARGIN,
    )
}

fn screen_top_left_point() -> Option<ObjcNSPoint> {
    let mtm = ObjcMainThreadMarker::new()?;
    let screen = NSScreen::mainScreen(mtm)?;
    let frame = screen.frame();
    let fallback_center_x = frame.origin.x + ((frame.size.width - ISLAND_WIDTH) / 2.0).max(0.0);
    let x = notch_panel_x(&screen).unwrap_or(fallback_center_x);

    Some(ObjcNSPoint::new(
        x,
        frame.origin.y + frame.size.height - TOP_MARGIN,
    ))
}

fn notch_panel_x(screen: &NSScreen) -> Option<f64> {
    let left_selector = objc2::sel!(auxiliaryTopLeftArea);
    let right_selector = objc2::sel!(auxiliaryTopRightArea);
    let supports_left: bool = unsafe { objc2::msg_send![screen, respondsToSelector: left_selector] };
    let supports_right: bool = unsafe { objc2::msg_send![screen, respondsToSelector: right_selector] };
    if !supports_left || !supports_right {
        return None;
    }

    let left: ObjcNSRect = unsafe { objc2::msg_send![screen, auxiliaryTopLeftArea] };
    let right: ObjcNSRect = unsafe { objc2::msg_send![screen, auxiliaryTopRightArea] };
    if left.size.width <= 0.0 || right.size.width <= 0.0 {
        return None;
    }

    let notch_left = left.origin.x + left.size.width;
    let notch_right = right.origin.x;
    if notch_right <= notch_left {
        return None;
    }

    Some(((notch_left + notch_right) / 2.0) - (ISLAND_WIDTH / 2.0))
}

fn position_panel(panel: &dyn Panel) {
    if let Some(point) = screen_top_left_point() {
        unsafe {
            let _: () = objc2::msg_send![panel.as_panel(), setFrameTopLeftPoint: point];
        }
    }
}

pub fn setup_island_panel(app: &AppHandle) -> tauri::Result<()> {
    let position = initial_panel_position(app);
    let panel = PanelBuilder::<_, IslandPanel>::new(app, ISLAND_WINDOW_LABEL)
        .url(WebviewUrl::App("index.html?surface=island".into()))
        .title("OctoMonitor Island")
        .position(Position::Logical(position))
        .size(Size::Logical(LogicalSize::new(ISLAND_WIDTH, ISLAND_HEIGHT)))
        .level(PanelLevel::Status)
        .floating(true)
        .has_shadow(false)
        .transparent(true)
        .opaque(false)
        .hides_on_deactivate(false)
        .works_when_modal(true)
        .collection_behavior(
            CollectionBehavior::new()
                .can_join_all_spaces()
                .full_screen_auxiliary()
                .stationary()
                .ignores_cycle(),
        )
        .style_mask(StyleMask::empty().borderless().nonactivating_panel())
        .no_activate(true)
        .with_window(|window| {
            window
                .decorations(false)
                .transparent(true)
                .background_color(Color(0, 0, 0, 0))
                .resizable(false)
                .shadow(false)
                .skip_taskbar(true)
                .visible(false)
                .focused(false)
        })
        .build()?;
    position_panel(panel.as_ref());
    panel.show();
    Ok(())
}

pub fn set_island_visible(app: &AppHandle, visible: bool) -> Result<(), String> {
    let panel = app
        .get_webview_panel(ISLAND_WINDOW_LABEL)
        .map_err(|_| "Island panel is not available".to_string())?;
    if visible {
        position_panel(panel.as_ref());
        panel.show();
    } else {
        panel.hide();
    }
    Ok(())
}

pub fn toggle_island(app: &AppHandle) -> Result<bool, String> {
    let panel = app
        .get_webview_panel(ISLAND_WINDOW_LABEL)
        .map_err(|_| "Island panel is not available".to_string())?;
    let next_visible = !panel.is_visible();
    if next_visible {
        position_panel(panel.as_ref());
        panel.show();
    } else {
        panel.hide();
    }
    Ok(next_visible)
}
