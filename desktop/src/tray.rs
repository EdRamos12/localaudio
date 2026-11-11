use std::cell::Cell;
use std::time::Duration;
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

pub enum Action {
    Open,
    Quit,
}

/// Tray icon that keeps LocalAudio streaming after its window is closed.
pub struct Tray {
    _icon: TrayIcon,
    open: MenuId,
    quit: MenuId,
    pub quitting: Cell<bool>,
}

impl Tray {
    pub fn new() -> anyhow::Result<Self> {
        let open = MenuItem::new("Open LocalAudio", true, None);
        let quit = MenuItem::new("Quit", true, None);
        let menu = Menu::new();
        menu.append_items(&[&open, &quit])?;
        let image = crate::ui::app_icon();
        let icon = TrayIconBuilder::new()
            .with_tooltip("LocalAudio")
            .with_icon(Icon::from_rgba(image.rgba, image.width, image.height)?)
            .with_menu(Box::new(menu))
            .build()?;
        Ok(Self {
            _icon: icon,
            open: open.id().clone(),
            quit: quit.id().clone(),
            quitting: Cell::new(false),
        })
    }

    pub fn poll(&self) -> Option<Action> {
        let mut action = None;
        for event in MenuEvent::receiver().try_iter() {
            if event.id == self.quit {
                return Some(Action::Quit);
            }
            if event.id == self.open {
                action = Some(Action::Open);
            }
        }
        // Drain everything: Windows floods the channel with hover/move events.
        for event in TrayIconEvent::receiver().try_iter() {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                action = Some(Action::Open);
            }
        }
        action
    }

    /// Blocks while the window is closed. Returns true to reopen it, false to quit.
    pub fn wait_for_open(&self) -> bool {
        while !self.quitting.get() {
            pump_messages();
            match self.poll() {
                Some(Action::Open) => return true,
                Some(Action::Quit) => return false,
                None => std::thread::sleep(Duration::from_millis(100)),
            }
        }
        false
    }
}

// Windows tray icons only receive clicks while their thread dispatches window messages.
#[cfg(windows)]
fn pump_messages() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
    };
    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

#[cfg(not(windows))]
fn pump_messages() {}
