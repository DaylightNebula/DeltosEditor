use deltos::{DeltosPlugin, editors::text::SimpleTextDeltosEditorPlugin};
use gearbox::{App, anyhow};

fn main() -> anyhow::Result<()> {
    App::new()
        .add_plugin(DeltosPlugin::default())
        .add_plugin(SimpleTextDeltosEditorPlugin)
        .run()
}
