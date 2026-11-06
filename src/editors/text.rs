use std::{path::PathBuf, sync::Arc};

use anarchy::*;
use gearbox::{App, Plugin, ui::*};

use crate::{DeltosEditor, colors, editors::FileEditor, sources::FileSource};

/// Marks the node holding an open file's text.
#[derive(Debug, Clone, Component)]
pub struct OpenFile {
    pub path: PathBuf,
    pub dirty: bool,
}

/// Plugin to initialize a simple text editor into the Deltos editor.
/// This editor will initialize as a default editor when no default is
/// set at the time this plugins startup system is run.
pub struct SimpleTextDeltosEditorPlugin;
impl Plugin for SimpleTextDeltosEditorPlugin {
    fn build(self, app: App) -> App {
        app.on_startup(|mut editor: ResMut<DeltosEditor>| {
            let text_editor = SimpleTextDeltosEditor { font: editor.font.clone() };
            editor.add_editor(Box::new(text_editor), true);
        })
    }
}

/// Edits any file as plain text.
/// (Not called `TextEditor`, because gearbox::ui already exports that name.)
#[derive(Debug)]
pub struct SimpleTextDeltosEditor {
    pub font: Arc<SDFFont>,
}

impl FileEditor for SimpleTextDeltosEditor {
    fn name(&self) -> String { "Text".into() }

    fn file_types(&self) -> Vec<String> {
        ["txt", "md", "toml", "json"].map(String::from).to_vec()
    }

    fn build_editor(&mut self, source: &dyn FileSource, path: PathBuf) -> UiBundle {
        let font = self.font.clone();
        match source.read_file_text(path.clone()) {
            Ok(text) => text_area(font, text, path),
            Err(err) => UiBundle::new(Node::new().padding(Rect::single(px(12.0))))
                .text(Text::new(font, &format!("Couldn't open {}: {err:#}", path.display()), colors::red, 14.0)),
        }
    }
}

fn text_area(font: Arc<SDFFont>, text: String, path: PathBuf) -> UiBundle {
    UiBundle::new(
        Node::new()
            .grow(1.0)
            .width(Val::Percent(1.0))
            .min_width(px(0.0))
            .padding(Rect::single(px(12.0)))
            .background(colors::bg0)
            .border(1.5, colors::bg4)
    )
    .text(Text::new(font, "", colors::fg0, 14.0))
    .with(TextInput::new(text).multiline())
    .with(OpenFile { path, dirty: false })
    .interactive()
}
