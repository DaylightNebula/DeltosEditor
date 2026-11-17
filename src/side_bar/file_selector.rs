use std::{path::{Path, PathBuf}, sync::Arc};

use anarchy::{Component, Event, Res, ResMut, SystemPin, World, pin};
use gearbox::{App, Plugin, anyhow, ui::*};
use uuid::Uuid;

use crate::{DeltosEditor, hover::HoverStyle, notify::NotifyLevel, side_bar::{SideBarElement, SideBarPlugin, SideBarSlot}, sources::{FileList, Source, SourceAction, SourceActionActivated, TaskResult, replace_placeholder}, window::{FileItem, FolderItem}};

pub const FOLDER_CLOSED: &str = "\u{ea83}";
pub const FOLDER_OPEN: &str = "\u{eaf7}";
pub const FILE_ICON: &str = "\u{eb60}";
/// Badge shown after the name of a read-only source.
pub const LOCK_ICON: &str = "\u{ea75}";
pub const SOURCE_GAP: f32 = 16.0;
pub const INDENT: f32 = 14.0;
/// Width of a source's root context menu, in logical pixels.
pub const ACTION_MENU_WIDTH: f32 = 200.0;

/// A source's root menu item in the file tree that sends `SourceActionActivated` when chosen.
///
/// Fields:
/// * source: ID of the source whose root menu holds the item.
/// * action: ID of the `SourceAction` the item stands for.
#[derive(Debug, Clone, Component)]
pub struct SourceActionItem {
    pub source: Uuid,
    pub action: String
}

/// Adds the `FileSelectorSideBar` `SideBarElement` and its supporting systems.
pub struct FileSelectorPlugin;
impl Plugin for FileSelectorPlugin {
    fn build(self, app: App) -> App {
        app .add_plugin(SideBarPlugin)
            .on_startup_with(|mut editor: ResMut<DeltosEditor>| {
                editor.add_side_bar(Box::new(FileSelectorSideBar));
            }, pin(SystemPin::Start))
            .on_render_update(open_folders)
            .on_render_update(activate_source_actions)
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
        let lists = sorted_sources(editor)
            .iter()
            .map(|source| source_list(editor, source))
            .collect::<Vec<_>>();

        UiBundle::new(Node::column()
            .min_width(Val::Percent(1.0))
            .max_width(Val::Percent(1.0))
            .height(Val::Percent(1.0)))
            .child(UiBundle::new(Node::column()
                .width(Val::Percent(1.0))
                .grow(1.0)
                .min_height(px(0.0))
                .gap(px(SOURCE_GAP))
                .overflow_y(Overflow::Scroll))
                .children(lists))
    }
}

/// Every source of the editor in the order the file tree shows them: by `Source::order`, then
/// by name.
///
/// # Parameters
/// * editor: Editor whose sources are sorted.
pub fn sorted_sources(editor: &DeltosEditor) -> Vec<Arc<dyn Source>> {
    let mut sources: Vec<_> = editor.sources
        .values()
        .cloned()
        .collect();
    sources.sort_by_cached_key(|source| (source.order(), source.name()));
    sources
}

/// Builds one source's list: a header with the source's icon, name and lock badge above its
/// root folder contents.  Right clicking the header opens the source's `root_actions`.
fn source_list(editor: &DeltosEditor, source: &Arc<dyn Source>) -> UiBundle {
    let title = match source.icon() {
        Some(icon) => format!("{icon} {}", source.name().to_uppercase()),
        None => source.name().to_uppercase(),
    };

    // the header: the title, with a lock badge pushed to the end for read-only sources
    let mut header = UiBundle::new(Node::row()
        .shrink(0.0)
        .align(AlignItems::Center)
        .padding(Rect::new(px(8.0), px(6.0), px(12.0), px(12.0))))
        .child(UiBundle::new(Node::new()
            .grow(1.0)
            .min_width(px(0.0)))
            .text(Text::new(editor.font.clone(), title, editor.theme.bg_5, 12.0)
                .with_align(Align::Start, Align::Center)));
    if source.read_only() {
        header = header.child(UiBundle::new(Node::new().shrink(0.0))
            .text(Text::new(editor.font.clone(), LOCK_ICON, editor.theme.bg_5, 12.0)
                .with_align(Align::End, Align::Center))
            .with(Tooltip("Read only".to_string())));
    }

    // right clicking the header lists the source's actions
    let actions = source.root_actions();
    if !actions.is_empty() {
        let items = actions
            .iter()
            .map(|action| action_menu_item(source.id(), action));
        header = header
            .interactive()
            .context_menu(themed::menu(ACTION_MENU_WIDTH, items));
    }

    UiBundle::new(Node::column().shrink(0.0))
        .child(header)
        .child(folder_contents(editor, source.id(), Path::new("."), 0))
}

/// Builds the item of a source's root context menu that chooses `action`.
fn action_menu_item(source: Uuid, action: &SourceAction) -> UiBundle {
    let label = match &action.icon {
        Some(icon) => format!("{icon} {}", action.label),
        None => action.label.clone(),
    };
    themed::menu_item(label)
        .with(SourceActionItem { source, action: action.id.clone() })
}

/// Sends `SourceActionActivated` for every root menu item chosen this frame.
fn activate_source_actions(world: World, activated: Event<Activate>, sent: Event<SourceActionActivated>) {
    for Activate { entity } in activated.read() {
        let Ok(Some(item)) = world.handle(entity).get::<&SourceActionItem>() else { continue };
        sent.send(SourceActionActivated { source: item.source, action: item.action.clone() });
    }
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
            // surface listing failures, like an unreachable source, beyond the tree's red row
            if let Err(err) = &list {
                editor.notify(NotifyLevel::Error, format!("{err:#}"));
            }
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
