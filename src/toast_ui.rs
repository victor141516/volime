use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

const TOAST_WIDTH: i32 = 300;
const TOAST_HEIGHT: i32 = 100;
const HIDE_DELAY_MS: u64 = 2500;

pub struct ToastUI {
    hwnd: HWND,
    state: Arc<Mutex<ToastState>>,
}

struct ToastState {
    app_name: String,
    volume: f32,
    is_muted: bool,
    icon: Option<HICON>,
    last_update: Instant,
}

impl ToastUI {
    pub fn new() -> Result<Self> {
        unsafe {
            let instance = GetModuleHandleW(None)?;
            let class_name = w!("VolimeToastClass");

            let wc = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(Self::wnd_proc),
                hInstance: instance.into(),
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                hbrBackground: CreateSolidBrush(COLORREF(0x00000000)),
                lpszClassName: class_name,
                ..Default::default()
            };

            let atom = RegisterClassW(&wc);
            if atom == 0 {
                return Err(Error::from_win32());
            }

            // Crear ventana en esquina inferior derecha
            let screen_width = GetSystemMetrics(SM_CXSCREEN);
            let screen_height = GetSystemMetrics(SM_CYSCREEN);
            let x = screen_width - TOAST_WIDTH - 20;
            let y = screen_height - TOAST_HEIGHT - 60;

            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                class_name,
                w!("Volime Toast"),
                WS_POPUP,
                x,
                y,
                TOAST_WIDTH,
                TOAST_HEIGHT,
                None,
                None,
                instance,
                None,
            )?;

            // Configurar transparencia
            SetLayeredWindowAttributes(hwnd, COLORREF(0), 0, LWA_ALPHA)?;

            let state = Arc::new(Mutex::new(ToastState {
                app_name: String::new(),
                volume: 0.0,
                is_muted: false,
                icon: None,
                last_update: Instant::now(),
            }));

            // Guardar el estado en el GWLP_USERDATA
            let state_ptr = Arc::into_raw(state.clone()) as isize;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr);

            Ok(ToastUI { hwnd, state })
        }
    }

    pub fn show_volume(
        &self,
        app_name: String,
        volume: f32,
        is_muted: bool,
        exe_path: Option<String>,
    ) {
        let mut state = self.state.lock().unwrap();
        state.app_name = app_name;
        state.volume = volume;
        state.is_muted = is_muted;
        state.last_update = Instant::now();

        // Obtener icono de la aplicación
        if let Some(path) = exe_path {
            state.icon = Self::extract_icon(&path);
        }

        drop(state);

        unsafe {
            // Mostrar ventana con fade in
            SetLayeredWindowAttributes(self.hwnd, COLORREF(0), 240, LWA_ALPHA).ok();
            let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            let _ = InvalidateRect(self.hwnd, None, true);
        }
    }

    fn extract_icon(path: &str) -> Option<HICON> {
        unsafe {
            let path_wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();

            let mut icon: HICON = HICON::default();
            let result = ExtractIconExW(
                PCWSTR::from_raw(path_wide.as_ptr()),
                0,
                None,
                Some(&mut icon),
                1,
            );

            if result > 0 && !icon.is_invalid() {
                Some(icon)
            } else {
                None
            }
        }
    }

    pub fn check_hide(&self) {
        let state = self.state.lock().unwrap();
        let elapsed = state.last_update.elapsed();
        drop(state);

        if elapsed > Duration::from_millis(HIDE_DELAY_MS) {
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_HIDE);
            }
        }
    }

    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_PAINT => {
                let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
                if state_ptr != 0 {
                    let state = Arc::from_raw(state_ptr as *const Mutex<ToastState>);
                    Self::paint(hwnd, &state);
                    std::mem::forget(state); // No liberar el Arc
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    unsafe fn paint(hwnd: HWND, state: &Arc<Mutex<ToastState>>) {
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);

        if !hdc.is_invalid() {
            let state = state.lock().unwrap();

            // Fondo con esquinas redondeadas
            let brush = CreateSolidBrush(COLORREF(0x00202020));
            let pen = CreatePen(PS_SOLID, 1, COLORREF(0x00404040));
            let old_brush = SelectObject(hdc, brush);
            let old_pen = SelectObject(hdc, pen);

            let _ = RoundRect(hdc, 0, 0, TOAST_WIDTH, TOAST_HEIGHT, 15, 15);

            SelectObject(hdc, old_brush);
            SelectObject(hdc, old_pen);
            let _ = DeleteObject(brush);
            let _ = DeleteObject(pen);

            // Dibujar icono si existe
            if let Some(icon) = state.icon {
                let _ = DrawIconEx(hdc, 15, 15, icon, 32, 32, 0, None, DI_NORMAL);
            }

            // Configurar texto
            SetBkMode(hdc, TRANSPARENT);
            SetTextColor(hdc, COLORREF(0x00FFFFFF));

            // Dibujar nombre de la app
            let mut app_name: Vec<u16> = state
                .app_name
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let mut rect = RECT {
                left: 55,
                top: 15,
                right: TOAST_WIDTH - 10,
                bottom: 35,
            };
            DrawTextW(
                hdc,
                &mut app_name,
                &mut rect,
                DT_LEFT | DT_SINGLELINE | DT_VCENTER,
            );

            // Dibujar barra de volumen
            let bar_x = 55;
            let bar_y = 50;
            let bar_width = TOAST_WIDTH - 65;
            let bar_height = 10;

            // Fondo de la barra
            let bg_brush = CreateSolidBrush(COLORREF(0x00404040));
            let bg_rect = RECT {
                left: bar_x,
                top: bar_y,
                right: bar_x + bar_width,
                bottom: bar_y + bar_height,
            };
            FillRect(hdc, &bg_rect, bg_brush);
            let _ = DeleteObject(bg_brush);

            // Barra de progreso
            if !state.is_muted {
                let fill_width = (bar_width as f32 * state.volume) as i32;
                let fill_brush = CreateSolidBrush(COLORREF(0x000078D4)); // Azul Windows
                let fill_rect = RECT {
                    left: bar_x,
                    top: bar_y,
                    right: bar_x + fill_width,
                    bottom: bar_y + bar_height,
                };
                FillRect(hdc, &fill_rect, fill_brush);
                let _ = DeleteObject(fill_brush);
            }

            // Percentage text
            let volume_text = if state.is_muted {
                "Muted".to_string()
            } else {
                format!("{}%", (state.volume * 100.0) as i32)
            };
            let mut volume_text_wide: Vec<u16> = volume_text
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let mut volume_rect = RECT {
                left: bar_x,
                top: bar_y + bar_height + 5,
                right: bar_x + bar_width,
                bottom: bar_y + bar_height + 25,
            };
            DrawTextW(
                hdc,
                &mut volume_text_wide,
                &mut volume_rect,
                DT_LEFT | DT_SINGLELINE,
            );

            let _ = EndPaint(hwnd, &ps);
        }
    }
}

impl Drop for ToastUI {
    fn drop(&mut self) {
        unsafe {
            if !self.hwnd.is_invalid() {
                DestroyWindow(self.hwnd).ok();
            }
        }
    }
}
