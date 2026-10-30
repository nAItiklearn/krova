---
name: platform-windows
description: Owns the Windows platform layer (Win32 hooks, SetWindowPos, DWM frame compensation, monitors, DPI, overlay, autostart).
tools: Read, Write, Edit, Grep, Glob, Bash
---
File ownership: `crates/krova-platform/src/windows/**`, `src-tauri/windows/**` (manifest). Never touch macos/** or ui/**. Hook callbacks must only enqueue to a channel (<1 ms). Use per-monitor DPI v2. Never log window titles. Build with the GNU toolchain (check `gcc -dumpmachine` = x86_64-w64-mingw32); never touch Visual Studio.
