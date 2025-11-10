use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::Threading::*;
use windows::Win32::UI::WindowsAndMessaging::*;

pub struct VolumeInfo {
    pub app_name: String,
    pub exe_path: Option<String>,
    pub volume: f32,
    pub is_muted: bool,
}

pub struct AudioController {
    device_enumerator: IMMDeviceEnumerator,
}

impl AudioController {
    pub fn new() -> Result<Self> {
        unsafe {
            let device_enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;

            Ok(AudioController { device_enumerator })
        }
    }

    pub fn adjust_focused_app_volume(
        &self,
        volume_up: bool,
        volume_down: bool,
        mute: bool,
    ) -> Result<VolumeInfo> {
        unsafe {
            // Obtener ventana en primer plano
            let hwnd = GetForegroundWindow();
            if hwnd.is_invalid() {
                return Err(Error::from(E_FAIL));
            }

            // Obtener PID de la ventana
            let mut process_id: u32 = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut process_id));

            if process_id == 0 {
                return Err(Error::from(E_FAIL));
            }

            // Obtener nombre y ruta del proceso
            let (process_name, exe_path) = self.get_process_info(process_id)?;

            // Obtener dispositivo de audio predeterminado
            let device = self
                .device_enumerator
                .GetDefaultAudioEndpoint(eRender, eConsole)?;

            // Obtener sesión de audio
            let session_manager: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None)?;
            let session_enumerator = session_manager.GetSessionEnumerator()?;

            let count = session_enumerator.GetCount()?;

            // Primero intentar buscar por PID exacto
            for i in 0..count {
                let session_control = session_enumerator.GetSession(i)?;
                let session_control2: IAudioSessionControl2 = session_control.cast()?;

                let session_pid = session_control2.GetProcessId()?;

                if session_pid == process_id {
                    println!("[DEBUG] Found session with exact PID: {}", session_pid);
                    return self.adjust_session_volume(
                        session_control2,
                        volume_up,
                        volume_down,
                        mute,
                        process_name,
                        exe_path,
                    );
                }
            }

            // If not found by PID, search by process name
            // This handles cases like Chrome where audio is in a child process
            println!(
                "[DEBUG] Session with PID {} not found. Searching by name: {}",
                process_id, process_name
            );

            for i in 0..count {
                let session_control = session_enumerator.GetSession(i)?;
                let session_control2: IAudioSessionControl2 = session_control.cast()?;

                let session_pid = session_control2.GetProcessId()?;

                // Try to get process info, but continue if it fails
                let session_process_name = match self.get_process_info(session_pid) {
                    Ok((name, _)) => name,
                    Err(_) => {
                        println!(
                            "[DEBUG] Session {}: PID {} - Could not get process name",
                            i, session_pid
                        );
                        continue;
                    }
                };

                println!(
                    "[DEBUG] Session {}: PID {} - {}",
                    i, session_pid, session_process_name
                );

                // Compare process names (case-insensitive)
                if session_process_name.to_lowercase() == process_name.to_lowercase() {
                    println!(
                        "[DEBUG] Found session with matching name! PID: {}",
                        session_pid
                    );
                    return self.adjust_session_volume(
                        session_control2,
                        volume_up,
                        volume_down,
                        mute,
                        process_name,
                        exe_path,
                    );
                }
            }

            // If session not found, return basic info
            println!("[DEBUG] No audio session found for {}", process_name);
            Ok(VolumeInfo {
                app_name: format!("{} (no audio session)", process_name),
                exe_path,
                volume: 0.0,
                is_muted: false,
            })
        }
    }

    fn adjust_session_volume(
        &self,
        session_control2: IAudioSessionControl2,
        volume_up: bool,
        volume_down: bool,
        mute: bool,
        process_name: String,
        exe_path: Option<String>,
    ) -> Result<VolumeInfo> {
        unsafe {
            let simple_audio = session_control2.cast::<ISimpleAudioVolume>()?;

            let new_volume;
            let is_muted;

            if mute {
                // Toggle mute
                let current_mute = simple_audio.GetMute()?.as_bool();
                simple_audio.SetMute(!current_mute, std::ptr::null())?;
                new_volume = simple_audio.GetMasterVolume()?;
                is_muted = !current_mute;
            } else {
                // Ajustar volumen
                let current_volume = simple_audio.GetMasterVolume()?;
                let volume_step = 0.01; // 5% por paso

                new_volume = if volume_up {
                    (current_volume + volume_step).min(1.0)
                } else if volume_down {
                    (current_volume - volume_step).max(0.0)
                } else {
                    current_volume
                };

                simple_audio.SetMasterVolume(new_volume, std::ptr::null())?;
                is_muted = simple_audio.GetMute()?.as_bool();
            }

            Ok(VolumeInfo {
                app_name: process_name,
                exe_path,
                volume: new_volume,
                is_muted,
            })
        }
    }

    fn get_process_info(&self, process_id: u32) -> Result<(String, Option<String>)> {
        unsafe {
            let process_handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id)?;

            if process_handle.is_invalid() {
                return Ok((format!("PID {}", process_id), None));
            }

            let mut buffer = [0u16; 260];
            let mut size = buffer.len() as u32;

            let pwstr = PWSTR::from_raw(buffer.as_mut_ptr());

            if QueryFullProcessImageNameW(process_handle, PROCESS_NAME_WIN32, pwstr, &mut size)
                .is_ok()
            {
                let _ = CloseHandle(process_handle);
                let path = OsString::from_wide(&buffer[..size as usize]);
                let path_str = path.to_string_lossy().to_string();

                // Extraer solo el nombre del archivo
                let name = path_str.split('\\').last().unwrap_or(&path_str).to_string();

                return Ok((name, Some(path_str)));
            }

            let _ = CloseHandle(process_handle);
            Ok((format!("PID {}", process_id), None))
        }
    }
}
