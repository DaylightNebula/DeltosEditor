use std::{path::PathBuf, sync::Arc};

use anarchy::*;
use gearbox::ui::*;

use crate::{colors, editors::FileEditor, sources::FileSource};

/// Marks the node holding an open file's text.
#[derive(Debug, Clone, Component)]
pub struct OpenFile {
    pub path: PathBuf,
    pub dirty: bool,
}

/// Edits any file as plain text.
/// (Not called `TextEditor`, because gearbox::ui already exports that name.)
pub struct PlainTextEditor {
    pub font: Arc<SDFFont>,
}

impl FileEditor for PlainTextEditor {
    fn name(&self) -> String { "Text".into() }

    fn file_types(&self) -> Vec<String> {
        ["txt", "md", "toml", "json"].map(String::from).to_vec()
    }

    fn build_editor<S: FileSource>(&mut self, source: S, path: PathBuf) -> UiBundle {
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
