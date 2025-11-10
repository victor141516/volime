# Volime

Per-application volume control for Windows using multimedia keyboard keys with a Windows 11-style floating interface.

## Features

- **Normal Mode**: Volume media keys (up, down, mute) work normally, controlling system volume.
- **Modifier Mode**: When pressing the modifier key (default: `Shift`) together with media keys, the volume adjusts specifically for the currently active application.
- **Floating UI**: When controlling an application's specific volume, a floating interface appears showing:
  - Application icon
  - Application name
  - Volume progress bar
  - Current volume percentage or "Muted" status
  - The interface automatically fades away after 2.5 seconds
- **System Tray Icon**: Right-click the tray icon to:
  - Change the modifier key (Shift/Control/Alt)
  - Exit the application
- **Embedded Icon**: The icon is embedded in the executable - no external files needed

## Requirements

- Windows 10 or later
- Rust (to compile from source)

## Installation

1. Clone this repository:

```bash
git clone <repo-url>
cd volime
```

2. Compile the project:

```bash
cargo build --release
```

3. The executable will be at `target/release/volime.exe`

## Usage

1. Run the program:

```bash
cargo run --release
```

Or run the compiled binary directly:

```bash
target/release/volime.exe
```

2. The program will run in the background:

   - **Without modifier key**: Media keys control system volume normally
   - **With modifier key**: Media keys control the active application's volume

3. To exit the program, press `Ctrl+C` in the terminal or right-click the tray icon and select "Exit"

## Supported Keys

- `Volume Up` / `Modifier + Volume Up`: Increase volume
- `Volume Down` / `Modifier + Volume Down`: Decrease volume
- `Volume Mute` / `Modifier + Volume Mute`: Mute/unmute

## Single Executable

The compiled program is a **single standalone executable** (~380 KB) with:

- All dependencies statically linked
- Icon embedded in the resources
- No external files or DLLs required
- Runs in the background without showing a console window
- Fully portable - just copy `volime.exe` and run

## Technical Details

The program uses:

- **Windows Keyboard Hook**: Intercepts multimedia keys globally
- **Windows Audio Session API**: Controls individual application volumes through the Windows audio mixer
- **GetForegroundWindow**: Detects the currently foreground application
- **Multi-process Support**: Automatically finds the correct process for applications like Chrome/Brave that use multiple processes

## Notes

- The program requires permissions to install a global keyboard hook
- Only works on Windows
- The application must be producing audio to appear in Windows audio mixer
- Works with most Windows applications including multi-process apps like web browsers

## License

MIT
