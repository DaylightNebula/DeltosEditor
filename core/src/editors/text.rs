use std::{path::PathBuf, sync::Arc};

use anarchy::*;
use gearbox::{App, Plugin, anyhow, ui::*};
use winit::keyboard::Key;

use crate::{DeltosEditor, editors::FileEditor, notify::NotifyLevel, sources::{BoxFuture, Source, TaskResult, is_conflict}, theme::Theme};
use uuid::Uuid;

/// Marks the node holding an open file's text.
///
/// Fields:
/// * source: ID of the `Source` the file was loaded from and is saved to.
/// * path: Path of the file inside its source.
/// * dirty: Whether the text has unsaved changes.
/// * read_only: Whether the file opened read only, so it is never saved.
/// * version: Version of the file in its source when it was loaded or last saved, if the
///   source tracks versions.  Saves only write over this version.
#[derive(Debug, Clone, Component)]
pub struct OpenFile {
    pub source: Uuid,
    pub path: PathBuf,
    pub dirty: bool,
    pub read_only: bool,
    pub version: Option<String>,
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

    fn build_editor(&mut self, theme: &Theme, source: Arc<dyn Source>, path: PathBuf, read_only: bool) -> BoxFuture<'static, UiBundle> {
        let (theme, font) = (*theme, self.font.clone());
        Box::pin(async move {
            match source.read_file_text_versioned(path.clone()).await {
                Ok((text, version)) => {
                    let file = OpenFile { source: source.id(), path, dirty: false, read_only, version };
                    text_area(&theme, font, text, file)
                }
                Err(err) => UiBundle::new(Node::new().padding(Rect::single(px(12.0))))
                    .text(Text::new(font, &format!("Couldn't open {}: {err:#}", path.display()), theme.red, 14.0)),
            }
        })
    }
}

/// Builds the text area for an open file, which can't be edited if the file is read only.
fn text_area(theme: &Theme, font: Arc<SDFFont>, text: String, file: OpenFile) -> UiBundle {
    let mut input = TextInput::new(text).multiline();
    input.disabled = file.read_only;

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
    .with(input)
    .with(file)
    .interactive()
}

/// Marks a file dirty when its text is edited.
fn mark_dirty(world: World, changed: Event<ValueChange<String>>) {
    for ValueChange { entity, .. } in changed.read() {
        if let Ok(Some(mut file)) = world.handle(entity).get::<&mut OpenFile>()
            && !file.read_only
        {
            file.dirty = true;
        }
    }
}

/// The source an open file is saved to, or `None` when it must not be saved: it has no
/// unsaved changes, or it or its source is read only.
///
/// # Parameters
/// * editor: Editor holding the file's source.
/// * file: The open file to save.
///
/// # Errors
/// * The file's source isn't in the editor anymore.
pub fn save_target(editor: &DeltosEditor, file: &OpenFile) -> anyhow::Result<Option<Arc<dyn Source>>> {
    if !file.dirty || file.read_only { return Ok(None) }
    let source = editor.sources
        .get(&file.source)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Couldn't save {}: its source is gone", file.path.display()))?;
    Ok((!source.read_only()).then_some(source))
}

/// Writes every dirty, writable file back to its source in the background when save is
/// pressed.  Writes only replace the version of the file that was loaded, so a file changed
/// remotely isn't overwritten.
fn save_files(mut editor: ResMut<DeltosEditor>, input: Res<UiInput>, mut files: Query<(&'static OpenFile, &'static TextInput)>) {
    let command = if cfg!(target_os = "macos") { input.modifiers.super_key() } else { input.modifiers.control_key() };
    let pressed_s = input.keys.iter().any(|(key, _)| matches!(key, Key::Character(c) if c.eq_ignore_ascii_case("s")));
    if !(command && pressed_s) { return }

    while let Ok(Some((entity, (file, field)))) = files.next() {
        let source = match save_target(&editor, &file) {
            Ok(Some(source)) => source,
            Ok(None) => continue,
            Err(err) => {
                editor.notify(NotifyLevel::Error, format!("{err:#}"));
                continue;
            }
        };

        // write the text, then mark the file saved if it wasn't edited while writing
        let (path, text, version) = (file.path.clone(), field.text.clone(), file.version.clone());
        editor.tasks.spawn(Box::pin(async move {
            let result = source.write_file_text_if(path.clone(), text.clone(), version).await;
            let apply: TaskResult = Box::new(move |world: &World, editor: &mut DeltosEditor| match result {
                Ok(version) => mark_saved(world, entity, &text, version),
                Err(err) if is_conflict(&err) => editor.notify(NotifyLevel::Warning,
                    format!("{} changed remotely, so it wasn't saved. Reopen it to see the new version.", path.display())),
                Err(err) => editor.notify(NotifyLevel::Error, format!("Couldn't save {}: {err:#}", path.display())),
            });
            apply
        }));
    }
}

/// Records a file's new version once `saved` is written, and clears its dirty flag unless its
/// text changed since.
fn mark_saved(world: &World, entity: EntityID, saved: &str, version: Option<String>) {
    let unchanged = world.handle(entity)
        .get::<&TextInput>()
        .ok()
        .flatten()
        .is_some_and(|field| field.text == saved);

    if let Ok(Some(mut file)) = world.handle(entity).get::<&mut OpenFile>() {
        file.version = version;
        if unchanged { file.dirty = false }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::memory::MemorySource;

    /// Builds an editor with the bundled font and the given sources.
    fn editor(sources: Vec<MemorySource>) -> DeltosEditor {
        let font = SDFFont::new(include_bytes!("../../fonts/BlexMonoNerdFontMono-Regular.ttf"))
            .expect("bundled font loads");
        let mut editor = DeltosEditor::new(Arc::new(font));
        for source in sources {
            editor.add_source(Box::new(source));
        }
        editor
    }

    /// An open, edited file from `source`.
    fn dirty_file(source: &MemorySource) -> OpenFile {
        OpenFile { source: source.id(), path: "a.txt".into(), dirty: true, read_only: false, version: None }
    }

    #[test]
    fn dirty_writable_files_are_saved() -> anyhow::Result<()> {
        let source = MemorySource::new("main");
        let editor = editor(vec![source.clone()]);

        let target = save_target(&editor, &dirty_file(&source))?;

        assert_eq!(target.map(|source| source.id()), Some(source.id()));
        Ok(())
    }

    #[test]
    fn read_only_sources_are_never_saved() -> anyhow::Result<()> {
        let source = MemorySource::new("gearbox").with_read_only(true);
        let editor = editor(vec![source.clone()]);

        assert!(save_target(&editor, &dirty_file(&source))?.is_none());
        Ok(())
    }

    #[test]
    fn read_only_and_clean_files_are_never_saved() -> anyhow::Result<()> {
        let source = MemorySource::new("main");
        let editor = editor(vec![source.clone()]);
        let read_only = OpenFile { read_only: true, ..dirty_file(&source) };
        let clean = OpenFile { dirty: false, ..dirty_file(&source) };

        assert!(save_target(&editor, &read_only)?.is_none());
        assert!(save_target(&editor, &clean)?.is_none());
        Ok(())
    }

    #[test]
    fn files_of_removed_sources_fail_to_save() {
        let source = MemorySource::new("main");
        let mut editor = editor(vec![source.clone()]);
        editor.remove_source(source.id());

        assert!(save_target(&editor, &dirty_file(&source)).is_err());
    }
}
