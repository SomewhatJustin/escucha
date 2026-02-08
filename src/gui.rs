use anyhow::Result;

pub fn run_gui() -> Result<()> {
    crate::gui_bridge::run_qml_app();
    Ok(())
}
