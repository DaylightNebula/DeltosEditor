use std::sync::Arc;

use anarchy::*;
use gearbox::{App, GearboxRenderPlugin, anyhow};
use gearbox::ui::*;

use crate::window::WindowPlugin;

pub mod colors;
pub mod editors;
pub mod render;
pub mod sources;
pub mod window;

/// The primary storage for the Detlos Editor.
#[derive(Resource, Debug)]
pub struct DeltosEditor {
    pub font: Arc<SDFFont>,
    pub tabs: Vec<String>
}

fn main() -> anyhow::Result<()> {
    let font = Arc::new(SDFFont::new(include_bytes!("../fonts/BlexMonoNerdFontMono-Regular.ttf"))?);
    App::new()
        .add_plugin(GearboxRenderPlugin)
        .add_plugin(UIPlugin)
        .add_plugin(WidgetPlugin::new(font.clone()))
        .add_plugin(WindowPlugin)
        .add_resource(DeltosEditor {
            font: font,
            tabs: vec![
                "main.rs".into(),
                "todo.md".into(),
                "Editor.java".into(),
                "cloning-utils.rs".into(),
            ]
        })
        .run()
}
