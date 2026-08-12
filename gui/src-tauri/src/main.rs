// A console window behind the app on Windows would be nothing but confusing.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    transcrate_gui_lib::run();
}
