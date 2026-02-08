use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
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
