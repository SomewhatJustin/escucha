#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("escucha/gui_bridge.h");

        fn run_qml_app() -> i32;
    }
}

pub fn run_qml_app() -> i32 {
    ffi::run_qml_app()
}
