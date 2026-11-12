use std::{path::PathBuf, sync::Arc};

use anarchy::*;
use gearbox::{App, Plugin, ui::*};
use winit::keyboard::Key;

use crate::{DeltosEditor, editors::FileEditor, sources::{BoxFuture, Source, TaskResult}, theme::Theme};
use uuid::Uuid;

/// Marks the node holding an open file's text.
///
/// Fields:
/// * source: ID of the `Source` the file was loaded from and is saved to.
/// * path: Path of the file inside its source.
/// * dirty: Whether the text has unsaved changes.
#[derive(Debug, Clone, Component)]
pub struct OpenFile {
    pub source: Uuid,
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

    fn build_editor(&mut self, theme: &Theme, source: Arc<dyn Source>, path: PathBuf) -> BoxFuture<'static, UiBundle> {
        let (theme, font) = (*theme, self.font.clone());
        Box::pin(async move {
            match source.read_file_text(path.clone()).await {
                Ok(text) => text_area(&theme, font, text, source.id(), path),
                Err(err) => UiBundle::new(Node::new().padding(Rect::single(px(12.0))))
                    .text(Text::new(font, &format!("Couldn't open {}: {err:#}", path.display()), theme.red, 14.0)),
            }
        })
    }
}

/// Builds the editable text area for a file loaded from `source`.
fn text_area(theme: &Theme, font: Arc<SDFFont>, text: String, source: Uuid, path: PathBuf) -> UiBundle {
    UiBundle::new(
        Node::new()
            .grow(1.0)
            .width(Val::Percent(1.0))
            .min_width(px(0.0))
            .min_height(px(0.0))
            .padding(Rect::single(px(12.0)))
            .background(theme.bg_0)
            .border(1.5, theme.bg_4)
    )
    .text(Text::new(font, "", theme.bg_8, 14.0))
    .with(TextInput::new(text).multiline())
    .with(OpenFile { source, path, dirty: false })
    .interactive()
}

fn mark_dirty(world: World, changed: Event<ValueChange<String>>) {
    for ValueChange { entity, .. } in changed.read() {
        if let Ok(Some(mut file)) = world.handle(entity).get::<&mut OpenFile>() {
            file.dirty = true;
        }
    }
}

/// Writes every dirty file back to its source in the background when save is pressed.
fn save_files(editor: Res<DeltosEditor>, input: Res<UiInput>, mut files: Query<(&'static OpenFile, &'static TextInput)>) {
    let command = if cfg!(target_os = "macos") { input.modifiers.super_key() } else { input.modifiers.control_key() };
    let pressed_s = input.keys.iter().any(|(key, _)| matches!(key, Key::Character(c) if c.eq_ignore_ascii_case("s")));
    if !(command && pressed_s) { return }

    while let Ok(Some((entity, (file, field)))) = files.next() {
        if !file.dirty { continue }
        let Some(source) = editor.sources.get(&file.source).cloned() else {
            log::error!("Couldn't save {}: unknown source {}", file.path.display(), file.source);
            continue;
        };

        // write the text, then mark the file saved if it wasn't edited while writing
        let (path, text) = (file.path.clone(), field.text.clone());
        editor.tasks.spawn(Box::pin(async move {
            let result = source.write_file_text(path, text.clone()).await;
            let apply: TaskResult = Box::new(move |world: &World, _: &mut DeltosEditor| match result {
                Ok(()) => mark_saved(world, entity, &text),
                Err(err) => log::error!("{err:#}"),
            });
            apply
        }));
    }
}

/// Clears a file's dirty flag once `saved` is written, unless its text changed since.
fn mark_saved(world: &World, entity: EntityID, saved: &str) {
    let unchanged = world.handle(entity)
        .get::<&TextInput>()
        .ok()
        .flatten()
        .is_some_and(|field| field.text == saved);
    if !unchanged { return }

    if let Ok(Some(mut file)) = world.handle(entity).get::<&mut OpenFile>() {
        file.dirty = false;
    }
}
