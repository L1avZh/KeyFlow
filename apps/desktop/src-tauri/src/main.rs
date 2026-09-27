// Prevents an additional console window from appearing on Windows in
// release builds; has no effect elsewhere.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    keyflow_desktop_lib::run();
}
