//! Windows notification-area controls for Classic Studio.
//!
//! Create this only after eframe's native event loop is running (for example,
//! in the `App` factory). Poll [`TrayController::poll_action`] from the app's
//! regular UI callback. On close requests, send `ViewportCommand::CancelClose`
//! and `ViewportCommand::Visible(false)` to keep the process alive; on `Show`,
//! send `Visible(true)` and `Focus`; on `Quit`, allow the normal eframe exit.
//!
//! This module is intentionally Windows-only. Put `tray-icon` in the Windows
//! target dependency table so Linux builds do not need GTK/AppIndicator.

use std::{
    error::Error,
    sync::mpsc::{self, Receiver},
};

use eframe::egui;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

const SHOW_ID: &str = "classic-studio.show";
const QUIT_ID: &str = "classic-studio.quit";

/// Actions the UI layer should apply to its eframe root viewport.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayAction {
    Show,
    Quit,
}

/// Owns the Windows tray icon for as long as the app should stay resident.
pub struct TrayController {
    // Dropping TrayIcon removes it from the notification area.
    _icon: TrayIcon,
    actions: Receiver<TrayAction>,
}

impl TrayController {
    /// Build the native tray menu and icon from straight RGBA pixels.
    ///
    /// Call on the main thread from eframe's app-creation callback. `rgba`
    /// must contain exactly `width * height * 4` bytes.
    pub fn new(
        ctx: egui::Context,
        rgba: Vec<u8>,
        width: u32,
        height: u32,
    ) -> Result<Self, Box<dyn Error>> {
        let icon = Icon::from_rgba(rgba, width, height)?;

        let (sender, actions) = mpsc::channel();
        let menu_sender = sender.clone();
        let menu_ctx = ctx.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let action = if event.id() == SHOW_ID {
                Some(TrayAction::Show)
            } else if event.id() == QUIT_ID {
                Some(TrayAction::Quit)
            } else {
                None
            };
            if let Some(action) = action {
                let _ = menu_sender.send(action);
                menu_ctx.request_repaint();
            }
        }));
        let tray_ctx = ctx;
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            let show = matches!(
                event,
                TrayIconEvent::DoubleClick {
                    button: tray_icon::MouseButton::Left,
                    ..
                } | TrayIconEvent::Click {
                    button: tray_icon::MouseButton::Left,
                    button_state: tray_icon::MouseButtonState::Up,
                    ..
                }
            );
            if show {
                let _ = sender.send(TrayAction::Show);
                tray_ctx.request_repaint();
            }
        }));

        let menu = Menu::new();
        let show = MenuItem::with_id(SHOW_ID, "Open Classic Studio", true, None);
        let quit = MenuItem::with_id(QUIT_ID, "Quit Classic Studio", true, None);
        menu.append(&show)?;
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&quit)?;

        let tray_icon = TrayIconBuilder::new()
            .with_icon(icon)
            .with_tooltip("Classic Studio")
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .build()?;

        Ok(Self {
            _icon: tray_icon,
            actions,
        })
    }

    /// Drain native tray messages and return the latest app-level action.
    ///
    /// The menu event channel is global in `tray-icon`; this app should have a
    /// single controller and should not install a separate global event handler.
    pub fn poll_action(&self) -> Option<TrayAction> {
        self.actions.try_iter().last()
    }
}
