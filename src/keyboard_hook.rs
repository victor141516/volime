use parking_lot::RwLock;
use std::sync::Arc;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::audio_control::AudioController;
use crate::system_tray::ModifierKey;
use crate::toast_ui::ToastUI;

static mut AUDIO_CONTROLLER: Option<Arc<AudioController>> = None;
static mut TOAST_UI: Option<Arc<ToastUI>> = None;
static mut MODIFIER_KEY: Option<Arc<RwLock<ModifierKey>>> = None;

pub struct KeyboardHook {
    hook: HHOOK,
}

impl KeyboardHook {
    pub fn install(
        audio_controller: Arc<AudioController>,
        toast_ui: Arc<ToastUI>,
        modifier_key: Arc<RwLock<ModifierKey>>,
    ) -> Result<Self> {
        unsafe {
            AUDIO_CONTROLLER = Some(audio_controller);
            TOAST_UI = Some(toast_ui);
            MODIFIER_KEY = Some(modifier_key);

            let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), None, 0)?;

            if hook.is_invalid() {
                return Err(Error::from_win32());
            }

            println!("Keyboard hook installed successfully");

            Ok(KeyboardHook { hook })
        }
    }
}

impl Drop for KeyboardHook {
    fn drop(&mut self) {
        unsafe {
            let _ = UnhookWindowsHookEx(self.hook);
            AUDIO_CONTROLLER = None;
            println!("Keyboard hook uninstalled");
        }
    }
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let kb = *(lparam.0 as *const KBDLLHOOKSTRUCT);
        let vk_code = kb.vkCode;

        // Solo procesar eventos WM_KEYDOWN
        if wparam.0 == WM_KEYDOWN as usize {
            // Obtener tecla modificadora configurada
            let modifier_vk = unsafe {
                let ptr = std::ptr::addr_of!(MODIFIER_KEY);
                if let Some(mod_key) = &*ptr {
                    mod_key.read().to_vk()
                } else {
                    VK_SHIFT.0 as i32 // Por defecto Shift
                }
            };

            // Verificar si la tecla modificadora está presionada
            let modifier_pressed = (GetAsyncKeyState(modifier_vk) as u16 & 0x8000) != 0;

            // Teclas multimedia de volumen
            let is_volume_up = vk_code == VK_VOLUME_UP.0 as u32;
            let is_volume_down = vk_code == VK_VOLUME_DOWN.0 as u32;
            let is_volume_mute = vk_code == VK_VOLUME_MUTE.0 as u32;

            if modifier_pressed && (is_volume_up || is_volume_down || is_volume_mute) {
                // Modifier + media key: control active app volume
                let controller_ptr = std::ptr::addr_of!(AUDIO_CONTROLLER);
                if let Some(controller) = &*controller_ptr {
                    let action = if is_volume_up {
                        "increase"
                    } else if is_volume_down {
                        "decrease"
                    } else {
                        "mute"
                    };

                    match controller.adjust_focused_app_volume(
                        is_volume_up,
                        is_volume_down,
                        is_volume_mute,
                    ) {
                        Ok(volume_info) => {
                            println!("Volume of '{}': {}", volume_info.app_name, action);

                            // Show toast UI
                            let toast_ptr = std::ptr::addr_of!(TOAST_UI);
                            if let Some(toast) = &*toast_ptr {
                                toast.show_volume(
                                    volume_info.app_name,
                                    volume_info.volume,
                                    volume_info.is_muted,
                                    volume_info.exe_path,
                                );
                            }
                        }
                        Err(e) => {
                            eprintln!("Error adjusting app volume: {}", e);
                        }
                    }
                }

                // Block key so it doesn't affect system volume
                return LRESULT(1);
            }
            // If no modifier key, let system handle the key normally
        }
    }

    CallNextHookEx(None, code, wparam, lparam)
}
