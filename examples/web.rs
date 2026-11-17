//! Deltos in the browser, with in-memory sources standing in for remote ones.
//!
//! Build it with `trunk serve` from the crate's root (see `Trunk.toml` and `index.html`), or
//! check it with `cargo build --target wasm32-unknown-unknown --example web`.  It also runs
//! natively with `cargo run --example web`.
//!
//! It shows the host extension points: root icons and ordering, a read-only module with a lock
//! badge, a root context menu and toasts.

use anarchy::{Event, ResMut};
use deltos::{DeltosEditor, DeltosPlugin, editors::text::SimpleTextDeltosEditorPlugin, notify::NotifyLevel, sources::{SourceAction, SourceActionActivated, memory::MemorySource}};
use gearbox::{App, anyhow};

/// Glyph of local modules.
const MODULE_ICON: &str = "\u{eaf7}";

/// Glyph of shared, locked modules.
const SHARED_ICON: &str = "\u{ea75}";

/// ID of the root menu item that removes a module.
const REMOVE_MODULE: &str = "remove-module";

/// ID of the root menu item that only shows a toast.
const SAY_HELLO: &str = "say-hello";

fn main() -> anyhow::Result<()> {
    App::new()
        .add_plugin(DeltosPlugin::default()
            .with_source(module("main", 0)
                .with_file("Cargo.toml", "[package]\nname = \"main\"\n")
                .with_file("src/main.txt", "fn main() {}\n"))
            .with_source(MemorySource::new("gearbox")
                .with_icon(SHARED_ICON)
                .with_order(2)
                .with_read_only(true)
                .with_file("Cargo.toml", "[package]\nname = \"gearbox\"\n")
                .with_root_action(SourceAction::new(SAY_HELLO, "Say hello"))))
        .add_plugin(SimpleTextDeltosEditorPlugin)
        .on_startup(|mut editor: ResMut<DeltosEditor>| {
            editor.title = "Deltos on the web".to_string();
        })
        .on_render_update(handle_actions)
        .run()
}

/// A writable local module that can be removed from its root menu.
fn module(name: &str, order: i32) -> MemorySource {
    MemorySource::new(name)
        .with_icon(MODULE_ICON)
        .with_order(order)
        .with_root_action(SourceAction::new(SAY_HELLO, "Say hello"))
        .with_root_action(SourceAction::new(REMOVE_MODULE, "Remove module"))
}

/// Removes modules and shows toasts as the module roots' menu actions are chosen.
fn handle_actions(mut editor: ResMut<DeltosEditor>, chosen: Event<SourceActionActivated>) {
    for SourceActionActivated { source, action } in chosen.read() {
        match (source, action.as_str()) {
            (id, REMOVE_MODULE) => {
                if let Some(removed) = editor.remove_source(id) {
                    editor.notify(NotifyLevel::Warning, format!("Removed {}", removed.name()));
                }
            }
            (id, SAY_HELLO) => {
                let name = editor.sources
                    .get(&id)
                    .map(|source| source.name())
                    .unwrap_or_default();
                editor.notify(NotifyLevel::Info, format!("Hello from {name}"));
            }
            _ => editor.notify(NotifyLevel::Error, format!("Unknown action {action}")),
        }
    }
}
