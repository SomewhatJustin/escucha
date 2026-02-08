use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    // Avoid Cargo's default whole-tree scan for build script fingerprinting.
    // This prevents unrelated unreadable directories (e.g. makepkg artifacts)
    // from breaking builds.
    for path in [
        "build.rs",
        "src/bridge.rs",
        "src/gui_bridge.rs",
        "src/gui_bridge.cpp",
        "src/qml/Main.qml",
        "include/escucha/gui_bridge.h",
        "Cargo.toml",
        "Cargo.lock",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }

    CxxQtBuilder::new()
        .qt_module("Widgets")
        .qml_module(QmlModule {
            uri: "io.github.escucha",
            rust_files: &["src/bridge.rs"],
            qml_files: &["src/qml/Main.qml"],
            ..Default::default()
        })
        .file("src/gui_bridge.rs")
        .cc_builder(|cc| {
            cc.file("src/gui_bridge.cpp");
            cc.include("include");
        })
        .build();
}
