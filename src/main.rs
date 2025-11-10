#![windows_subsystem = "windows"]

use parking_lot::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use windows::core::*;
use windows::Win32::System::Com::*;
use windows::Win32::UI::WindowsAndMessaging::*;

mod audio_control;
mod keyboard_hook;
mod system_tray;
mod toast_ui;

use audio_control::AudioController;
use keyboard_hook::KeyboardHook;
use system_tray::{ModifierKey, SystemTray};
use toast_ui::ToastUI;

fn main() -> Result<()> {
    println!("Volime - Per-Application Volume Control");
    println!("========================================");
    println!("Usage:");
    println!("  - Normal media keys: Control system volume");
    println!("  - Modifier + media keys: Control active app volume");
    println!("Press Ctrl+C to exit\n");

    // Initialize COM
    unsafe {
        let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
        if hr.is_err() {
            eprintln!("Error initializing COM");
        }
    };

    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    // Configure Ctrl+C handler
    ctrlc::set_handler(move || {
        println!("\nClosing Volime...");
        running_clone.store(false, Ordering::SeqCst);
    })
    .expect("Error configuring Ctrl+C handler");

    // Create modifier key (default Shift)
    let modifier_key = Arc::new(RwLock::new(ModifierKey::Shift));

    // Create system tray
    let _system_tray = SystemTray::new(modifier_key.clone(), running.clone())?;

    // Create audio controller
    let audio_controller = Arc::new(AudioController::new()?);

    // Create toast UI
    let toast_ui = Arc::new(ToastUI::new()?);

    // Install keyboard hook
    let hook = KeyboardHook::install(
        audio_controller.clone(),
        toast_ui.clone(),
        modifier_key.clone(),
    )?;

    println!("Initial modifier key: {}", modifier_key.read().to_string());
    println!("Right-click the tray icon to change settings\n");

    // Main loop
    unsafe {
        let mut msg = MSG::default();
        while running.load(Ordering::SeqCst) {
            // Process all pending messages
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_QUIT {
                    running.store(false, Ordering::SeqCst);
                    break;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            // Check if toast should be hidden
            toast_ui.check_hide();

            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    // Uninstall hook
    drop(hook);

    unsafe { CoUninitialize() };

    println!("Volime closed.");
    Ok(())
}
