use deltos::{DeltosPlugin, editors::text::SimpleTextDeltosEditorPlugin, sources::local::LocalFileSource};
use gearbox::{App, anyhow};

fn main() -> anyhow::Result<()> {
    App::new()
        .add_plugin(DeltosPlugin::default().with_source(LocalFileSource))
        .add_plugin(SimpleTextDeltosEditorPlugin)
        .run()
}
