use std::{path::PathBuf, sync::Arc};

use anarchy::*;
use gearbox::{App, Plugin, ui::*};
use winit::keyboard::Key;

use crate::{DeltosEditor, colors, editors::FileEditor, sources::{Source, local::LocalFileSource}};

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
        app .on_startup(|mut editor: ResMut<DeltosEditor>| {
                let text_editor = SimpleTextDeltosEditor { font: editor.font.clone() };
                editor.add_editor(Box::new(text_editor), true);
            })
            .on_render_update(mark_dirty)
            .on_render_update(save_files)
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

    fn build_editor(&mut self, source: &dyn Source, path: PathBuf) -> UiBundle {
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

fn mark_dirty(world: World, changed: Event<ValueChange<String>>) {
    for ValueChange { entity, .. } in changed.read() {
        if let Ok(Some(mut file)) = world.handle(entity).get::<&mut OpenFile>() {
            file.dirty = true;
        }
    }
}

fn save_files(input: Res<UiInput>, mut files: Query<(&'static mut OpenFile, &'static TextInput)>) {
    let command = if cfg!(target_os = "macos") { input.modifiers.super_key() } else { input.modifiers.control_key() };
    let pressed_s = input.keys.iter().any(|(key, _)| matches!(key, Key::Character(c) if c.eq_ignore_ascii_case("s")));
    if !(command && pressed_s) { return }

    while let Ok(Some((_, (mut file, field)))) = files.next() {
        if !file.dirty { continue }
        match LocalFileSource.write_file_text(file.path.clone(), field.text.clone()) {
            Ok(()) => file.dirty = false,
            Err(err) => log::error!("{err:#}"),
        }
    }
}
