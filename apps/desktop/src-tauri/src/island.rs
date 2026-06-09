use objc2_app_kit::NSScreen;
use objc2_foundation::{
    MainThreadMarker as ObjcMainThreadMarker, NSEdgeInsets as ObjcNSEdgeInsets,
    NSPoint as ObjcNSPoint, NSRect as ObjcNSRect,
};
use std::{
    sync::{Arc, Mutex, OnceLock},
    thread,
    time::Duration,
};
use tauri::{
    window::Color, AppHandle, LogicalPosition, LogicalSize, Manager, Position, Size, WebviewUrl,
};
use tauri_nspanel::{
    tauri_panel, CollectionBehavior, ManagerExt, Panel, PanelBuilder, PanelLevel, StyleMask,
};

pub const ISLAND_WINDOW_LABEL: &str = "island";

const ISLAND_PANEL_WIDTH: f64 = 640.0;
const ISLAND_PANEL_HEIGHT: f64 = 560.0;
const EXTERNAL_CLOSED_WIDTH: f64 = 360.0;
const EXTERNAL_CLOSED_HEIGHT: f64 = 38.0;
const NOTCHED_CLOSED_HEIGHT_FALLBACK: f64 = 32.0;
const NOTCH_SIDE_RESERVE: f64 = 44.0;
const NOTCH_WIDTH_BLEED: f64 = 4.0;
const HOVER_POLL_INTERVAL: Duration = Duration::from_millis(16);
const ISLAND_EXPANSION_EVENT: &str = "octomonitor-island-expansion";

static ISLAND_HOVER_BOUNDS: OnceLock<Arc<Mutex<IslandHoverBounds>>> = OnceLock::new();

#[derive(Clone, Copy)]
pub enum IslandPositionMode {
    Auto,
    TopCenter,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct IslandChromeGeometry {
    closed_width: f64,
    closed_height: f64,
    notched: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct IslandHoverBounds {
    closed_x: f64,
    closed_y: f64,
    closed_width: f64,
    closed_height: f64,
    expanded_x: f64,
    expanded_y: f64,
    expanded_width: f64,
    expanded_height: f64,
}

impl IslandHoverBounds {
    fn from_top_left(top_left: ObjcNSPoint, geometry: IslandChromeGeometry) -> Self {
        let expanded_x = top_left.x;
        let expanded_y = top_left.y - ISLAND_PANEL_HEIGHT;
        Self {
            closed_x: expanded_x + ((ISLAND_PANEL_WIDTH - geometry.closed_width) / 2.0),
            closed_y: top_left.y - geometry.closed_height,
            closed_width: geometry.closed_width,
            closed_height: geometry.closed_height,
            expanded_x,
            expanded_y,
            expanded_width: ISLAND_PANEL_WIDTH,
            expanded_height: ISLAND_PANEL_HEIGHT,
        }
    }

    fn contains_closed(&self, point: ObjcNSPoint) -> bool {
        contains_point(
            point,
            self.closed_x,
            self.closed_y,
            self.closed_width,
            self.closed_height,
        )
    }

    fn contains_expanded(&self, point: ObjcNSPoint) -> bool {
        contains_point(
            point,
            self.expanded_x,
            self.expanded_y,
            self.expanded_width,
            self.expanded_height,
        )
    }
}

tauri_panel! {
    panel!(IslandPanel {
        config: {
            can_become_key_window: false,
            can_become_main_window: false,
            is_floating_panel: true
        }
    })
}

fn contains_point(point: ObjcNSPoint, x: f64, y: f64, width: f64, height: f64) -> bool {
    point.x >= x && point.x <= x + width && point.y >= y && point.y <= y + height
}

fn clicked_outside_expanded_panel(
    bounds: IslandHoverBounds,
    point: ObjcNSPoint,
    expanded: bool,
    buttons_pressed: bool,
    last_buttons_pressed: bool,
) -> bool {
    expanded && buttons_pressed && !last_buttons_pressed && !bounds.contains_expanded(point)
}

fn initial_panel_position(app: &AppHandle) -> LogicalPosition<f64> {
    let Some(monitor) = app.primary_monitor().ok().flatten() else {
        return LogicalPosition::new(0.0, 0.0);
    };
    let scale = monitor.scale_factor();
    let position = monitor.position();
    let size = monitor.size();
    let monitor_x = position.x as f64 / scale;
    let monitor_y = position.y as f64 / scale;
    let monitor_width = size.width as f64 / scale;

    LogicalPosition::new(
        monitor_x + ((monitor_width - ISLAND_PANEL_WIDTH) / 2.0).max(0.0),
        monitor_y,
    )
}

pub fn parse_position_mode(value: Option<&str>) -> Result<IslandPositionMode, String> {
    match value.unwrap_or("auto") {
        "auto" => Ok(IslandPositionMode::Auto),
        "topCenter" => Ok(IslandPositionMode::TopCenter),
        _ => Err("Unsupported island position".into()),
    }
}

fn island_hover_bounds(mode: IslandPositionMode) -> Option<IslandHoverBounds> {
    let mtm = ObjcMainThreadMarker::new()?;
    let screen = NSScreen::mainScreen(mtm)?;
    let top_left = screen_top_left_point(mode)?;
    let geometry = island_chrome_geometry(&screen);
    Some(IslandHoverBounds::from_top_left(top_left, geometry))
}

fn update_hover_bounds(mode: IslandPositionMode) {
    let Some(bounds) = island_hover_bounds(mode) else {
        return;
    };
    if let Some(state) = ISLAND_HOVER_BOUNDS.get() {
        if let Ok(mut current) = state.lock() {
            *current = bounds;
        }
    }
}

fn screen_top_left_point(mode: IslandPositionMode) -> Option<ObjcNSPoint> {
    let mtm = ObjcMainThreadMarker::new()?;
    let screen = NSScreen::mainScreen(mtm)?;
    let frame = screen.frame();
    let fallback_center_x =
        frame.origin.x + ((frame.size.width - ISLAND_PANEL_WIDTH) / 2.0).max(0.0);
    let x = match mode {
        IslandPositionMode::Auto => notch_center_x(&screen)
            .map(|center_x| center_x - (ISLAND_PANEL_WIDTH / 2.0))
            .unwrap_or(fallback_center_x),
        IslandPositionMode::TopCenter => fallback_center_x,
    };

    Some(ObjcNSPoint::new(x, frame.origin.y + frame.size.height))
}

fn notch_center_x(screen: &NSScreen) -> Option<f64> {
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

    Some((notch_left + notch_right) / 2.0)
}

fn chrome_geometry_from_parts(
    frame_width: f64,
    safe_area_top: f64,
    left_aux_width: f64,
    right_aux_width: f64,
) -> IslandChromeGeometry {
    if left_aux_width > 0.0 && right_aux_width > 0.0 {
        let notch_width =
            (frame_width - left_aux_width - right_aux_width + NOTCH_WIDTH_BLEED).max(0.0);
        return IslandChromeGeometry {
            closed_width: notch_width + (NOTCH_SIDE_RESERVE * 2.0),
            closed_height: safe_area_top.max(NOTCHED_CLOSED_HEIGHT_FALLBACK),
            notched: true,
        };
    }

    IslandChromeGeometry {
        closed_width: EXTERNAL_CLOSED_WIDTH,
        closed_height: EXTERNAL_CLOSED_HEIGHT,
        notched: false,
    }
}

fn island_chrome_geometry(screen: &NSScreen) -> IslandChromeGeometry {
    let frame = screen.frame();
    let safe_area: ObjcNSEdgeInsets = unsafe { objc2::msg_send![screen, safeAreaInsets] };
    let left: ObjcNSRect = unsafe { objc2::msg_send![screen, auxiliaryTopLeftArea] };
    let right: ObjcNSRect = unsafe { objc2::msg_send![screen, auxiliaryTopRightArea] };

    chrome_geometry_from_parts(
        frame.size.width,
        safe_area.top.max(left.size.height).max(right.size.height),
        left.size.width,
        right.size.width,
    )
}

fn island_url() -> WebviewUrl {
    let Some(mtm) = ObjcMainThreadMarker::new() else {
        return WebviewUrl::App("index.html?surface=island".into());
    };
    let Some(screen) = NSScreen::mainScreen(mtm) else {
        return WebviewUrl::App("index.html?surface=island".into());
    };

    let geometry = island_chrome_geometry(&screen);
    WebviewUrl::App(
        format!(
            "index.html?surface=island&closedWidth={:.0}&closedHeight={:.0}&notched={}",
            geometry.closed_width,
            geometry.closed_height,
            if geometry.notched { 1 } else { 0 }
        )
        .into(),
    )
}

fn position_panel(panel: &dyn Panel, mode: IslandPositionMode) {
    if let Some(point) = screen_top_left_point(mode) {
        unsafe {
            let _: () = objc2::msg_send![panel.as_panel(), setFrameTopLeftPoint: point];
        }
    }
}

fn configure_panel_mouse_tracking(panel: &dyn Panel) {
    unsafe {
        let _: () = objc2::msg_send![panel.as_panel(), setAcceptsMouseMovedEvents: true];
    }
}

fn dispatch_island_expansion(app: &AppHandle, expanded: bool, immediate: bool) -> bool {
    let Some(window) = app.get_webview_window(ISLAND_WINDOW_LABEL) else {
        return false;
    };
    let expanded = if expanded { "true" } else { "false" };
    let immediate = if immediate { "true" } else { "false" };
    window
        .eval(format!(
            "window.__OCTOMONITOR_ISLAND_EXPANDED__ = {expanded}; \
             window.dispatchEvent(new CustomEvent('{ISLAND_EXPANSION_EVENT}', {{ detail: {{ expanded: {expanded}, immediate: {immediate} }} }}));"
        ))
        .is_ok()
}

fn start_hover_monitor(app: AppHandle, bounds: Arc<Mutex<IslandHoverBounds>>) {
    thread::spawn(move || {
        let mut last_expanded: Option<bool> = None;
        let mut last_buttons_pressed = false;
        loop {
            let point = objc2_app_kit::NSEvent::mouseLocation();
            let buttons_pressed = objc2_app_kit::NSEvent::pressedMouseButtons() != 0;
            let Ok(bounds) = bounds.lock().map(|bounds| *bounds) else {
                thread::sleep(HOVER_POLL_INTERVAL);
                last_buttons_pressed = buttons_pressed;
                continue;
            };
            let clicked_outside_expanded = clicked_outside_expanded_panel(
                bounds,
                point,
                last_expanded == Some(true),
                buttons_pressed,
                last_buttons_pressed,
            );
            let should_expand = bounds.contains_closed(point)
                || (last_expanded == Some(true) && bounds.contains_expanded(point));

            if clicked_outside_expanded {
                if dispatch_island_expansion(&app, false, true) {
                    last_expanded = Some(false);
                }
            } else if last_expanded != Some(should_expand)
                && dispatch_island_expansion(&app, should_expand, false)
            {
                last_expanded = Some(should_expand);
            }

            last_buttons_pressed = buttons_pressed;
            thread::sleep(HOVER_POLL_INTERVAL);
        }
    });
}

pub fn setup_island_panel(app: &AppHandle) -> tauri::Result<()> {
    let position = initial_panel_position(app);
    let panel = PanelBuilder::<_, IslandPanel>::new(app, ISLAND_WINDOW_LABEL)
        .url(island_url())
        .title("OctoMonitor Island")
        .position(Position::Logical(position))
        .size(Size::Logical(LogicalSize::new(
            ISLAND_PANEL_WIDTH,
            ISLAND_PANEL_HEIGHT,
        )))
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
    configure_panel_mouse_tracking(panel.as_ref());
    position_panel(panel.as_ref(), IslandPositionMode::Auto);
    if let Some(bounds) = island_hover_bounds(IslandPositionMode::Auto) {
        let bounds = Arc::new(Mutex::new(bounds));
        let _ = ISLAND_HOVER_BOUNDS.set(bounds.clone());
        start_hover_monitor(app.clone(), bounds);
    }
    panel.show();
    Ok(())
}

pub fn set_island_visible(app: &AppHandle, visible: bool) -> Result<(), String> {
    set_island_visible_with_position(app, visible, IslandPositionMode::Auto)
}

pub fn set_island_visible_with_position(
    app: &AppHandle,
    visible: bool,
    position: IslandPositionMode,
) -> Result<(), String> {
    let panel = app
        .get_webview_panel(ISLAND_WINDOW_LABEL)
        .map_err(|_| "Island panel is not available".to_string())?;
    if visible {
        position_panel(panel.as_ref(), position);
        update_hover_bounds(position);
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
        position_panel(panel.as_ref(), IslandPositionMode::Auto);
        update_hover_bounds(IslandPositionMode::Auto);
        panel.show();
    } else {
        panel.hide();
    }
    Ok(next_visible)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notched_geometry_wraps_camera_housing_with_side_reserve() {
        let geometry = chrome_geometry_from_parts(1728.0, 32.0, 771.5, 771.5);

        assert_eq!(
            geometry,
            IslandChromeGeometry {
                closed_width: 277.0,
                closed_height: 32.0,
                notched: true,
            }
        );
    }

    #[test]
    fn external_geometry_uses_vibe_top_bar_fallback() {
        let geometry = chrome_geometry_from_parts(1920.0, 0.0, 0.0, 0.0);

        assert_eq!(
            geometry,
            IslandChromeGeometry {
                closed_width: EXTERNAL_CLOSED_WIDTH,
                closed_height: EXTERNAL_CLOSED_HEIGHT,
                notched: false,
            }
        );
    }

    #[test]
    fn hover_bounds_cover_collapsed_island_and_expanded_panel() {
        let bounds = IslandHoverBounds::from_top_left(
            ObjcNSPoint::new(544.0, 1117.0),
            IslandChromeGeometry {
                closed_width: 277.0,
                closed_height: 32.0,
                notched: true,
            },
        );

        assert!(bounds.contains_closed(ObjcNSPoint::new(740.0, 1101.0)));
        assert!(bounds.contains_closed(ObjcNSPoint::new(864.0, 1101.0)));
        assert!(bounds.contains_expanded(ObjcNSPoint::new(864.0, 900.0)));
        assert!(!bounds.contains_closed(ObjcNSPoint::new(1120.0, 1101.0)));
        assert!(!bounds.contains_expanded(ObjcNSPoint::new(1200.0, 900.0)));
    }

    #[test]
    fn outside_click_dismisses_only_when_expanded_and_press_starts_outside() {
        let bounds = IslandHoverBounds::from_top_left(
            ObjcNSPoint::new(544.0, 1117.0),
            IslandChromeGeometry {
                closed_width: 277.0,
                closed_height: 32.0,
                notched: true,
            },
        );

        assert!(clicked_outside_expanded_panel(
            bounds,
            ObjcNSPoint::new(100.0, 900.0),
            true,
            true,
            false,
        ));
        assert!(!clicked_outside_expanded_panel(
            bounds,
            ObjcNSPoint::new(864.0, 900.0),
            true,
            true,
            false,
        ));
        assert!(!clicked_outside_expanded_panel(
            bounds,
            ObjcNSPoint::new(100.0, 900.0),
            false,
            true,
            false,
        ));
        assert!(!clicked_outside_expanded_panel(
            bounds,
            ObjcNSPoint::new(100.0, 900.0),
            true,
            true,
            true,
        ));
    }
}
