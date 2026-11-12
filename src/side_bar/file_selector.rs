use std::path::{Path, PathBuf};

use anarchy::{Event, Res, ResMut, SystemPin, World, pin};
use gearbox::{App, Plugin, anyhow, ui::*};
use uuid::Uuid;

use crate::{DeltosEditor, hover::HoverStyle, side_bar::{SideBarElement, SideBarPlugin, SideBarSlot}, sources::{FileList, TaskResult, replace_placeholder}, window::{FileItem, FolderItem}};

pub const FOLDER_CLOSED: &str = "\u{ea83}";
pub const FOLDER_OPEN: &str = "\u{eaf7}";
pub const FILE_ICON: &str = "\u{eb60}";
pub const SOURCE_GAP: f32 = 16.0;
pub const INDENT: f32 = 14.0;

/// Adds the `FileSelectorSideBar` `SideBarElement` and its supporting systems.
pub struct FileSelectorPlugin;
impl Plugin for FileSelectorPlugin {
    fn build(self, app: App) -> App {
        app .add_plugin(SideBarPlugin)
            .on_startup_with(|mut editor: ResMut<DeltosEditor>| {
                editor.add_side_bar(Box::new(FileSelectorSideBar));
            }, pin(SystemPin::Start))
            .on_render_update(open_folders)
    }
}

/// Adds a file selector to the left side bar of the `DeltosEditor`.
#[derive(Debug)]
pub struct FileSelectorSideBar;
impl SideBarElement for FileSelectorSideBar {
    fn name(&self) -> String { "File Selector".to_string() }
    fn icon(&self) -> String { FILE_ICON.to_string() }
    fn default_slot(&self) -> SideBarSlot { SideBarSlot::Left }

    fn build(&self, _world: &World, editor: &DeltosEditor) -> UiBundle {
        let mut sources: Vec<_> = editor.sources.iter().collect();
        sources.sort_by_key(|(_, source)| source.name());

        UiBundle::new(Node::column()
            .min_width(Val::Percent(1.0))
            .max_width(Val::Percent(1.0))
            .height(Val::Percent(1.0))
            .gap(px(SOURCE_GAP))
            .overflow_y(Overflow::Scroll))
            .children(sources
                .into_iter()
                .map(|(id, source)| source_list(editor, *id, &source.name()))
                .collect::<Vec<_>>())
    }
}

/// Builds one source's list: a header with the source's name above its root folder contents.
fn source_list(editor: &DeltosEditor, source: Uuid, name: &str) -> UiBundle {
    UiBundle::new(Node::column().shrink(0.0))
        .child(UiBundle::new(Node::new()
            .shrink(0.0)
            .padding(Rect::new(px(8.0), px(6.0), px(12.0), px(12.0))))
            .text(Text::new(editor.font.clone(), name.to_uppercase(), editor.theme.bg_5, 12.0)
                .with_align(Align::Start, Align::Center)))
        .child(folder_contents(editor, source, Path::new("."), 0))
}

/// Builds a placeholder row for a folder's contents, and lists the folder from its source in
/// the background to replace the placeholder with the folder's items.
fn folder_contents(editor: &DeltosEditor, source: Uuid, path: &Path, depth: usize) -> UiBundle {
    let Some(file_source) = editor.sources.get(&source).cloned() else {
        return error_row(editor, depth, format!("Unknown source {source}"));
    };

    // list the folder, then swap its items in for the placeholder
    let (path, placeholder) = (path.to_path_buf(), reserve_id());
    editor.tasks.spawn(Box::pin(async move {
        let list = file_source.list_files(path).await;
        let apply: TaskResult = Box::new(move |world: &World, editor: &mut DeltosEditor| {
            replace_placeholder(world, placeholder, folder_items(editor, source, list, depth));
        });
        apply
    }));

    row(depth)
        .with_id(placeholder)
        .text(Text::new(editor.font.clone(), "Loading...", editor.theme.bg_5, 12.0)
            .with_align(Align::Start, Align::Center))
}

/// Builds the rows of a listed folder, or an error row if it couldn't be listed.
fn folder_items(editor: &DeltosEditor, source: Uuid, list: anyhow::Result<FileList>, depth: usize) -> Vec<UiBundle> {
    let list = match list {
        Ok(list) => list,
        Err(err) => return vec![error_row(editor, depth, format!("{err:#}"))],
    };

    let folders = list.folders.iter().map(|name| folder_item(editor, source, name.clone(), depth));
    let files = list.files.iter().map(|file| file_item(editor, source, file, depth));
    folders.chain(files).collect()
}

/// Builds a row showing an error in red.
fn error_row(editor: &DeltosEditor, depth: usize, message: String) -> UiBundle {
    row(depth).text(Text::new(editor.font.clone(), message, editor.theme.red, 12.0))
}

fn folder_item(editor: &DeltosEditor, source: Uuid, path: PathBuf, depth: usize) -> UiBundle {
    let name = path.file_name()
        .map(|a| a.to_string_lossy().to_string())
        .unwrap_or_else(|| String::new());

    UiBundle::new(Node::column().shrink(0.0))
        .with(Collapsible { open: false })
        .with(FolderItem { source, path, depth, loaded: false })
        .child(row(depth)
            .text(Text::new(editor.font.clone(), format!("{FOLDER_CLOSED} {name}"), editor.theme.bg_8, 14.0)
                .with_align(Align::Start, Align::Center))
            .interactive()
            .with(HoverStyle::new(editor.theme.bg_4)))
        .child(UiBundle::new(Node::column().shrink(0.0)))
}

fn file_item(editor: &DeltosEditor, source: Uuid, file: &PathBuf, depth: usize) -> UiBundle {
    let name = file.file_name()
        .map(|a| a.to_string_lossy().to_string())
        .unwrap_or_else(|| String::new());

    row(depth)
        .text(Text::new(editor.font.clone(), format!("{FILE_ICON} {name}"), editor.theme.bg_8, 14.0)
            .with_align(Align::Start, Align::Center))
        .interactive()
        .with(FileItem { source, path: file.clone() })
        .with(HoverStyle::new(editor.theme.bg_4))
}

fn row(depth: usize) -> UiBundle {
    UiBundle::new(Node::new()
        .shrink(0.0)
        .padding(Rect::new(px(6.0), px(6.0), px(16.0 + depth as f32 * INDENT), px(0.0))))
}

/// Lists a folder's contents the first time it opens, and swaps its header's icons as it opens
/// and closes. gearbox's `Collapsible` already shows and hides the contents.
pub fn open_folders(world: World, editor: Res<DeltosEditor>, toggled: Event<ValueChange<bool>>) {
    for ValueChange { entity, value: open } in toggled.read() {
        let Ok(Some(mut folder)) = world.handle(entity).get::<&mut FolderItem>() else { continue };
        let load = open && !folder.loaded;
        folder.loaded |= open;
        let (source, path, depth) = (folder.source, folder.path.clone(), folder.depth);

        // release the component before spawning into the world
        drop(folder);

        let children = world.hierarchy().children(entity).map(|children| children.to_vec()).unwrap_or_default();
        let [header, content, ..] = children[..] else { continue };

        if let Ok(Some(mut node)) = world.handle(header).get::<&mut Node>()
            && let Some(text) = node.style_mut().text_mut()
        {
            let (from, to) = if open { (FOLDER_CLOSED, FOLDER_OPEN) } else { (FOLDER_OPEN, FOLDER_CLOSED) };
            text.content = text.content.replacen(from, to, 1);
        }

        if load {
            world.spawn_with_parent(Some(content), folder_contents(&editor, source, &path, depth + 1).into_entity_builder());
        }
    }
}
