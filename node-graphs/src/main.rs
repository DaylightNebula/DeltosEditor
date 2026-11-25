use deltos::{DeltosPlugin, editors::text::SimpleTextDeltosEditorPlugin};
use gearbox::{App, anyhow};

/// Opens the editor on the local file system, which only exists natively.  On the web the
/// editor starts without sources; see the `web` example for one with sources.
fn main() -> anyhow::Result<()> {
    let plugin = DeltosPlugin::default();

    #[cfg(not(target_arch = "wasm32"))]
    let plugin = plugin.with_source(deltos::sources::local::LocalFileSource);

    App::new()
        .add_plugin(plugin)
        .add_plugin(SimpleTextDeltosEditorPlugin)
        .run()
}
