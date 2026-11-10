use anarchy::ResMut;
use deltos::{DeltosEditor, DeltosPlugin, editors::text::SimpleTextDeltosEditorPlugin, sources::local::LocalFileSource};
use gearbox::{App, anyhow};

fn main() -> anyhow::Result<()> {
    App::new()
        .add_plugin(DeltosPlugin::default())
        .add_plugin(SimpleTextDeltosEditorPlugin)
        .on_startup(setup_editor)
        .run()
}

fn setup_editor(mut editor: ResMut<DeltosEditor>) {
    editor.add_source(Box::new(LocalFileSource));
}
