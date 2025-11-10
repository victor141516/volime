use parking_lot::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

const WM_TRAYICON: u32 = WM_USER + 1;
const IDM_EXIT: u32 = 1001;
const IDM_MODIFIER_SHIFT: u32 = 1002;
const IDM_MODIFIER_CTRL: u32 = 1003;
const IDM_MODIFIER_ALT: u32 = 1004;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ModifierKey {
    Shift,
    Control,
    Alt,
}

impl ModifierKey {
    pub fn to_vk(&self) -> i32 {
        match self {
            ModifierKey::Shift => VK_SHIFT.0 as i32,
            ModifierKey::Control => VK_CONTROL.0 as i32,
            ModifierKey::Alt => VK_MENU.0 as i32,
        }
    }

    pub fn to_string(&self) -> &str {
        match self {
            ModifierKey::Shift => "Shift",
            ModifierKey::Control => "Control",
            ModifierKey::Alt => "Alt",
        }
    }
}

pub struct SystemTray {
    hwnd: HWND,
    _modifier_key: Arc<RwLock<ModifierKey>>,
    _running: Arc<AtomicBool>,
}

impl SystemTray {
    pub fn new(modifier_key: Arc<RwLock<ModifierKey>>, running: Arc<AtomicBool>) -> Result<Self> {
        unsafe {
            let instance = GetModuleHandleW(None)?;
            let class_name = w!("VolimeTrayClass");

            let wc = WNDCLASSW {
                lpfnWndProc: Some(Self::wnd_proc),
                hInstance: instance.into(),
                lpszClassName: class_name,
                ..Default::default()
            };

            let atom = RegisterClassW(&wc);
            if atom == 0 {
                return Err(Error::from_win32());
            }

            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class_name,
                w!("Volime"),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                None,
                None,
                instance,
                None,
            )?;

            // Guardar punteros usando propiedades de ventana
            let modifier_ptr = Arc::into_raw(modifier_key.clone()) as *mut std::ffi::c_void;
            SetPropW(hwnd, w!("modifier_key"), HANDLE(modifier_ptr))?;

            let running_ptr = Arc::into_raw(running.clone()) as *mut std::ffi::c_void;
            SetPropW(hwnd, w!("running"), HANDLE(running_ptr))?;

            let mut nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: hwnd,
                uID: 1,
                uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
                uCallbackMessage: WM_TRAYICON,
                hIcon: Self::load_icon()?,
                ..Default::default()
            };

            let tip = w!("Volime - Volume Control");
            let tip_bytes = tip.as_wide();
            let copy_len = tip_bytes.len().min(nid.szTip.len() - 1);
            nid.szTip[..copy_len].copy_from_slice(&tip_bytes[..copy_len]);

            let result = Shell_NotifyIconW(NIM_ADD, &nid);
            if !result.as_bool() {
                return Err(Error::from_win32());
            }

            println!("System tray icon created");

            Ok(SystemTray {
                hwnd,
                _modifier_key: modifier_key,
                _running: running,
            })
        }
    }

    fn load_icon() -> Result<HICON> {
        unsafe {
            // Try to load embedded icon from resources (ID 1)
            let instance = GetModuleHandleW(None)?;
            let hicon = LoadIconW(instance, PCWSTR::from_raw(1 as *const u16));

            if let Ok(icon) = hicon {
                if !icon.is_invalid() {
                    println!("Loaded embedded icon from resources");
                    return Ok(icon);
                }
            }

            // Fallback to default application icon
            println!("Using default system icon");
            LoadIconW(None, IDI_APPLICATION)
        }
    }

    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_TRAYICON => {
                let event = lparam.0 as u32;
                println!(
                    "[DEBUG TRAY] WM_TRAYICON message received, event: 0x{:X}",
                    event
                );

                if event == WM_RBUTTONUP || event == WM_RBUTTONDOWN {
                    println!("[DEBUG TRAY] Right click detected!");

                    let modifier_handle = GetPropW(hwnd, w!("modifier_key"));
                    let running_handle = GetPropW(hwnd, w!("running"));

                    let modifier_ptr = modifier_handle.0 as isize;
                    let running_ptr = running_handle.0 as isize;

                    println!(
                        "[DEBUG TRAY] modifier_ptr: {}, running_ptr: {}",
                        modifier_ptr, running_ptr
                    );

                    if modifier_ptr != 0 && running_ptr != 0 {
                        let modifier_key =
                            Arc::from_raw(modifier_ptr as *const RwLock<ModifierKey>);
                        let running = Arc::from_raw(running_ptr as *const AtomicBool);

                        println!("[DEBUG TRAY] Showing context menu...");
                        Self::show_context_menu(hwnd, &modifier_key, &running);

                        std::mem::forget(modifier_key);
                        std::mem::forget(running);
                    } else {
                        println!("[DEBUG TRAY] ERROR: Invalid pointers!");
                    }
                }
                LRESULT(0)
            }
            WM_COMMAND => {
                let command = (wparam.0 & 0xFFFF) as u32;

                let modifier_handle = GetPropW(hwnd, w!("modifier_key"));
                let running_handle = GetPropW(hwnd, w!("running"));

                let modifier_ptr = modifier_handle.0 as isize;
                let running_ptr = running_handle.0 as isize;

                if modifier_ptr != 0 && running_ptr != 0 {
                    let modifier_key = Arc::from_raw(modifier_ptr as *const RwLock<ModifierKey>);
                    let running = Arc::from_raw(running_ptr as *const AtomicBool);

                    match command {
                        IDM_EXIT => {
                            println!("Exiting from tray menu...");
                            running.store(false, Ordering::SeqCst);
                            PostQuitMessage(0);
                        }
                        IDM_MODIFIER_SHIFT => {
                            *modifier_key.write() = ModifierKey::Shift;
                            println!("Modifier key changed to: Shift");
                        }
                        IDM_MODIFIER_CTRL => {
                            *modifier_key.write() = ModifierKey::Control;
                            println!("Modifier key changed to: Control");
                        }
                        IDM_MODIFIER_ALT => {
                            *modifier_key.write() = ModifierKey::Alt;
                            println!("Modifier key changed to: Alt");
                        }
                        _ => {}
                    }

                    std::mem::forget(modifier_key);
                    std::mem::forget(running);
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    unsafe fn show_context_menu(
        hwnd: HWND,
        modifier_key: &Arc<RwLock<ModifierKey>>,
        _running: &Arc<AtomicBool>,
    ) {
        let menu = CreatePopupMenu().unwrap();
        let current_modifier = *modifier_key.read();

        // Submenu for modifier key
        let modifier_menu = CreatePopupMenu().unwrap();

        let shift_flags = if current_modifier == ModifierKey::Shift {
            MF_STRING | MF_CHECKED
        } else {
            MF_STRING
        };
        AppendMenuW(
            modifier_menu,
            shift_flags,
            IDM_MODIFIER_SHIFT as usize,
            w!("Shift"),
        )
        .ok();

        let ctrl_flags = if current_modifier == ModifierKey::Control {
            MF_STRING | MF_CHECKED
        } else {
            MF_STRING
        };
        AppendMenuW(
            modifier_menu,
            ctrl_flags,
            IDM_MODIFIER_CTRL as usize,
            w!("Control"),
        )
        .ok();

        let alt_flags = if current_modifier == ModifierKey::Alt {
            MF_STRING | MF_CHECKED
        } else {
            MF_STRING
        };
        AppendMenuW(
            modifier_menu,
            alt_flags,
            IDM_MODIFIER_ALT as usize,
            w!("Alt"),
        )
        .ok();

        // Add submenu to main menu
        AppendMenuW(
            menu,
            MF_STRING | MF_POPUP,
            modifier_menu.0 as usize,
            w!("Modifier Key"),
        )
        .ok();

        AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null()).ok();
        AppendMenuW(menu, MF_STRING, IDM_EXIT as usize, w!("Exit")).ok();

        let mut pt = POINT { x: 0, y: 0 };
        let _ = GetCursorPos(&mut pt);

        let _ = SetForegroundWindow(hwnd);

        let _ = TrackPopupMenu(menu, TPM_RIGHTBUTTON, pt.x, pt.y, 0, hwnd, None);

        let _ = DestroyMenu(menu);
    }
}

impl Drop for SystemTray {
    fn drop(&mut self) {
        unsafe {
            let nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: self.hwnd,
                uID: 1,
                ..Default::default()
            };

            let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
            let _ = DestroyWindow(self.hwnd);

            println!("System tray icon removed");
        }
    }
}
