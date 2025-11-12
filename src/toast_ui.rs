use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

const TOAST_WIDTH: i32 = 194;
const TOAST_HEIGHT: i32 = 52;
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

            // Obtener DPI del monitor principal para escalar correctamente
            let dpi = GetDpiForSystem();
            let scale = dpi as f32 / 96.0; // 96 es el DPI estándar

            // Escalar dimensiones según DPI
            let scaled_width = (TOAST_WIDTH as f32 * scale) as i32;
            let scaled_height = (TOAST_HEIGHT as f32 * scale) as i32;
            let scaled_radius = (12.0 * scale) as i32;

            // Crear ventana centrada en la parte inferior
            let screen_width = GetSystemMetrics(SM_CXSCREEN);
            let screen_height = GetSystemMetrics(SM_CYSCREEN);
            let x = (screen_width - scaled_width) / 2;
            let y = screen_height - scaled_height - (150.0 * scale) as i32;

            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                class_name,
                w!("Volime Toast"),
                WS_POPUP,
                x,
                y,
                scaled_width,
                scaled_height,
                None,
                None,
                instance,
                None,
            )?;

            // Aplicar región con esquinas redondeadas escaladas según DPI
            let region = CreateRoundRectRgn(
                0,
                0,
                scaled_width,
                scaled_height,
                scaled_radius,
                scaled_radius,
            );
            SetWindowRgn(hwnd, region, true);

            // Habilitar sombra suave usando class style
            let current_style = GetClassLongPtrW(hwnd, GCL_STYLE) as isize;
            let new_style = current_style | CS_DROPSHADOW.0 as isize;
            SetClassLongPtrW(hwnd, GCL_STYLE, new_style);

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

            // Obtener escalado DPI
            let dpi = GetDpiForSystem();
            let scale = dpi as f32 / 96.0;

            // Escalar dimensiones
            let scaled_width = (TOAST_WIDTH as f32 * scale) as i32;
            let scaled_height = (TOAST_HEIGHT as f32 * scale) as i32;
            let scaled_radius = (12.0 * scale) as i32;

            // Fondo con esquinas redondeadas escaladas
            let brush = CreateSolidBrush(COLORREF(0x00282828));
            let pen = CreatePen(PS_SOLID, 1, COLORREF(0x00404040));
            let old_brush = SelectObject(hdc, brush);
            let old_pen = SelectObject(hdc, pen);

            let _ = RoundRect(
                hdc,
                0,
                0,
                scaled_width,
                scaled_height,
                scaled_radius,
                scaled_radius,
            );

            SelectObject(hdc, old_brush);
            SelectObject(hdc, old_pen);
            let _ = DeleteObject(brush);
            let _ = DeleteObject(pen);

            // Dibujar icono centrado verticalmente a la izquierda escalado
            if let Some(icon) = state.icon {
                let icon_size = (24.0 * scale) as i32;
                let icon_x = (10.0 * scale) as i32;
                let icon_y = (scaled_height - icon_size) / 2;
                let _ = DrawIconEx(
                    hdc, icon_x, icon_y, icon, icon_size, icon_size, 0, None, DI_NORMAL,
                );
            }

            // Configurar texto
            SetBkMode(hdc, TRANSPARENT);
            SetTextColor(hdc, COLORREF(0x00CCCCCC)); // Gris claro en lugar de blanco puro

            // Crear fuente Calibri escalada según DPI
            let font_height = -(15.0 * scale) as i32; // Altura negativa para fuentes TrueType
            let font_name: Vec<u16> = "Segoe UI Variable\0".encode_utf16().collect();
            let font = CreateFontW(
                font_height,
                0,
                0,
                0,
                FW_NORMAL.0 as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET.0 as u32,
                OUT_DEFAULT_PRECIS.0 as u32,
                CLIP_DEFAULT_PRECIS.0 as u32,
                CLEARTYPE_QUALITY.0 as u32,
                (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
                PCWSTR::from_raw(font_name.as_ptr()),
            );
            let old_font = SelectObject(hdc, font);

            // Dibujar barra de volumen en el centro escalada
            let bar_x = (45.0 * scale) as i32;
            let bar_y = (scaled_height - (4.0 * scale) as i32) / 2;
            let bar_width = scaled_width - (85.0 * scale) as i32;
            let bar_height = (4.0 * scale) as i32;

            // Fondo de la barra
            let bg_brush = CreateSolidBrush(COLORREF(0x00AAAAAA));
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
                let fill_brush = CreateSolidBrush(COLORREF(0x00FFCE4E));
                let fill_rect = RECT {
                    left: bar_x,
                    top: bar_y,
                    right: bar_x + fill_width,
                    bottom: bar_y + bar_height,
                };
                FillRect(hdc, &fill_rect, fill_brush);
                let _ = DeleteObject(fill_brush);
            }

            // Texto de volumen a la derecha de la barra escalado
            let volume_text = if state.is_muted {
                "M".to_string()
            } else {
                format!("{}", (state.volume * 100.0) as i32)
            };
            let mut volume_text_wide: Vec<u16> = volume_text
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let mut volume_rect = RECT {
                left: bar_x + bar_width + (10.0 * scale) as i32,
                top: bar_y - (5.0 * scale) as i32,
                right: scaled_width - (5.0 * scale) as i32,
                bottom: bar_y + (12.0 * scale) as i32,
            };
            DrawTextW(
                hdc,
                &mut volume_text_wide,
                &mut volume_rect,
                DT_CENTER | DT_SINGLELINE | DT_VCENTER,
            );

            // Restaurar y limpiar fuente
            SelectObject(hdc, old_font);
            let _ = DeleteObject(font);

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
