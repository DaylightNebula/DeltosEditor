use std::fmt::{Debug, Formatter};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anarchy::*;
use gearbox::egui::egui::ahash::AHashMap;
use gearbox::{App, GearboxRenderPlugin, Plugin};
use gearbox::ui::*;
use uuid::{Uuid, uuid};
use winit::dpi::LogicalSize;

use crate::editors::FileEditor;
use crate::hover::HoverPlugin;
use crate::notify::{Notification, NotifyLevel, NotifyPlugin};
use crate::side_bar::{SideBarElement, SideBarSlot};
use crate::side_bar::file_selector::{FileSelectorPlugin, FileSelectorSideBar};
use crate::sources::{Source, SourceTasks, TaskResult, replace_placeholder};
use crate::theme::Theme;
use crate::top_bar::TopBarElement;
use crate::top_bar::title_decor::TitleDecorElement;
use crate::top_bar::window_decor::TopWindowDecorElement;
use crate::tiles::{DropZone, TileLayout};
use crate::window::{EditorContainer, EditorContent, TabStrip, TileArea, WindowPlugin};

pub const SIDE_BAR_UUID_NAMESPACE: Uuid = uuid!("4927bdc9-bdc3-499c-9a05-1e1645a311a5");
pub const TOP_BAR_UUID_NAMESPACE: Uuid = uuid!("db204c32-f0fd-4b55-991a-6fd4ed77f102");
pub const SOURCE_UUID_NAMESPACE: Uuid = uuid!("c007d5db-1320-45a4-9f70-163706d055ea");
pub const EDITOR_UUID_NAMESPACE: Uuid = uuid!("dcf01b57-1883-4daf-a843-49e6a0bfdc27");
pub const TAB_UUID_NAMESPACE: Uuid = uuid!("fefe4e8b-7e6b-4e1b-a728-249b6b052d71");

pub mod editors;
pub mod hover;
pub mod notify;
pub mod render;
pub mod side_bar;
pub mod sources;
pub mod theme;
pub mod tiles;
pub mod top_bar;
pub mod window;

/// The primary plugin for `Deltos` editors.  This initializes all core
/// requirements for the `DeltosEditor`.
///
/// Fields:
/// * font: Font used by the whole editor.
/// * sources: Sources added to the `DeltosEditor` when it is created, so they are available
///   before the editor's UI is first built.
pub struct DeltosPlugin { font: Arc<SDFFont>, sources: Vec<Box<dyn Source>> }

impl DeltosPlugin {
    /// Create a new `DeltosPlugin` that uses the given font
    /// for the editor.  If you do not have a font to provide
    /// use the `default` function.
    pub fn new(font: SDFFont) -> Self {
        Self { font: Arc::new(font), sources: Vec::new() }
    }

    /// Adds a source that the `DeltosEditor` starts with.
    ///
    /// Sources added here are registered before any startup system runs, so the file tree
    /// shows them when the editor's UI is first built.
    ///
    /// # Parameters
    /// * source: The source to add to the editor.
    pub fn with_source(mut self, source: impl Source + 'static) -> Self {
        self.sources.push(Box::new(source));
        self
    }
}

impl Default for DeltosPlugin {
    fn default() -> Self {
        // we are not worried about the error that could come from here as all errors could come
        // from errors that will come from examples and tests as the resource is embedded.
        let font = SDFFont::new(include_bytes!("../fonts/BlexMonoNerdFontMono-Regular.ttf")).unwrap();
        Self::new(font)
    }
}

impl Plugin for DeltosPlugin {
    fn build(self, app: App) -> App {
        // build the editor with every source given to this plugin
        let mut editor = DeltosEditor::new(self.font.clone());
        for source in self.sources {
            editor.add_source(source);
        }

        app .add_plugin(GearboxRenderPlugin)
            .add_plugin(UIPlugin)
            .add_plugin(WidgetPlugin::new(self.font.clone()))
            .add_plugin(WindowPlugin)
            .add_plugin(HoverPlugin)
            .add_plugin(FileSelectorPlugin)
            .add_plugin(NotifyPlugin)
            .add_resource(editor)
            .on_render_update(apply_finished_tasks)
            .with_window_attributes(|attribs| {
                attribs.with_inner_size(LogicalSize::new(1600, 850))
            })
            .on_startup(|mut editor: ResMut<DeltosEditor>| {
                editor.add_top_bar(Box::new(TitleDecorElement));
                editor.add_top_bar(Box::new(TopWindowDecorElement));
            })
    }
}

/// The primary storage for the Detlos Editor.
///
/// Fields:
/// * title: Title of the editor's window.
/// * font: Font used by the whole editor.
/// * theme: Colors used by the whole editor.
/// * sources: Every `Source` files can be opened from, by ID.
/// * top_bars: Every `TopBarElement` shown in the top bar, by ID.
/// * side_bars: Every `SideBarElement` shown in the side bars, by ID.
/// * editors: Every `FileEditor` tabs can be opened with, by ID.
/// * tiles: Every open `EditorTile`, by ID.
/// * layout: How `tiles` are arranged in the editor area.
/// * focused_tile: ID of the tile new tabs open in, the last one a tab was focused or dropped in.
/// * file_type_editor_lookup: ID of the `FileEditor` used for each file extension.
/// * default_editor: ID of the `FileEditor` used for files no other editor handles.
/// * tasks: Background work, like `Source` calls, whose results are applied to the world.
/// * notifications: Messages passed to `notify` that aren't shown as toasts yet.
/// * side_bar_rebuilds: Side bars whose shown element is rebuilt on the next frame, see
///   `request_rebuild`.
/// * tiles_dirty: Whether the editor area is rebuilt on the next frame, after tabs were closed
///   without access to the world (see `remove_source`).
#[derive(Resource, Debug)]
pub struct DeltosEditor {
    pub title: String,
    pub font: Arc<SDFFont>,
    pub theme: Theme,

    pub sources: AHashMap<Uuid, Arc<dyn Source>>,
    pub top_bars: AHashMap<Uuid, Box<dyn TopBarElement>>,
    pub side_bars: AHashMap<Uuid, Box<dyn SideBarElement>>,
    pub editors: AHashMap<Uuid, Box<dyn FileEditor>>,

    pub tiles: AHashMap<Uuid, EditorTile>,
    pub layout: TileLayout,
    pub focused_tile: Uuid,
    pub file_type_editor_lookup: AHashMap<String, Uuid>,
    pub default_editor: Option<Uuid>,
    pub tasks: SourceTasks,

    pub(crate) notifications: Vec<Notification>,
    pub(crate) side_bar_rebuilds: Vec<SideBarSlot>,
    pub(crate) tiles_dirty: bool
}

/// The ID of the tab showing a file, unique per source and path, so equal paths in different
/// sources (like two modules' `Cargo.toml`) open in their own tabs.
///
/// # Parameters
/// * source: ID of the `Source` the file belongs to.
/// * path: Path of the file inside its source.
pub fn tab_id(source: Uuid, path: &Path) -> Uuid {
    let mut name = source.as_bytes().to_vec();
    name.extend_from_slice(path.as_os_str().as_encoded_bytes());
    Uuid::new_v5(&TAB_UUID_NAMESPACE, &name)
}

/// Applies the results of every finished background task, see `SourceTasks`, then the UI
/// rebuilds they or other systems requested.
///
/// Takes the `DeltosEditor` mutably so it never runs alongside a system that is building UI
/// a task's result replaces.
fn apply_finished_tasks(world: World, mut editor: ResMut<DeltosEditor>) {
    for apply in editor.tasks.finished() {
        apply(&world, &mut editor);
    }

    // rebuild the editor area once tabs were closed, like by removing a source
    if editor.tiles_dirty {
        editor.tiles_dirty = false;
        editor.rebuild_tiles(&world);
    }

    // rebuild each requested side bar once, however often it was requested
    let mut slots = std::mem::take(&mut editor.side_bar_rebuilds);
    slots.sort();
    slots.dedup();
    for slot in slots {
        side_bar::rebuild_slot(&world, &editor, slot);
    }
}

/// A container of tabs that can be built into one tile.
///
/// Fields:
/// * id: Unique ID of this tile.
/// * tabs: Every tab open in this tile, by ID.
/// * editor_tab: ID of the tab whose editor is shown in this tile, if any.
#[derive(Debug)]
pub struct EditorTile {
    pub id: Uuid,
    pub tabs: AHashMap<Uuid, EditorTab>,
    pub editor_tab: Option<Uuid>
}

/// A container of all information of an active editor.
/// Brings together a tabs information and there parent `FileEditor`.
///
/// Fields:
/// * id: Unique ID of this tab.
/// * name: Display name of this tab.
/// * source: ID of the `Source` the tab's file is loaded from.
/// * path: Path of the tab's file inside its source.
/// * editor: ID of the `FileEditor` used for the tab, if any.
pub struct EditorTab {
    pub id: Uuid,
    pub name: String,
    pub source: Uuid,
    pub path: PathBuf,
    pub editor: Option<Uuid>
}

impl Debug for EditorTab {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditorTab")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("source", &self.source)
            .finish()
    }
}

impl DeltosEditor {
    pub fn new(font: Arc<SDFFont>) -> Self {
        let default_tile = EditorTile { id: Uuid::new_v4(), tabs: AHashMap::default(), editor_tab: None };
        let (layout, focused_tile) = (TileLayout::Tile(default_tile.id), default_tile.id);
        let mut tiles = AHashMap::default();
        tiles.insert(default_tile.id, default_tile);

        Self {
            font, tiles, layout, focused_tile,
            title: "Deltos Editor".to_string(),
            theme: Theme::default(),
            sources: AHashMap::default(),
            editors: AHashMap::default(),
            top_bars: AHashMap::default(),
            side_bars: AHashMap::default(),
            file_type_editor_lookup: AHashMap::default(),
            default_editor: None,
            tasks: SourceTasks::default(),
            notifications: Vec::new(),
            side_bar_rebuilds: Vec::new(),
            tiles_dirty: false
        }
    }

    /// Add an editor to this `DeltosEditor`.
    pub fn add_editor(&mut self, editor: Box<dyn FileEditor>, can_be_default: bool) {
        if self.default_editor.is_none() && can_be_default {
            self.default_editor = Some(editor.id());
        }

        for file_type in editor.file_types() {
            self.file_type_editor_lookup.insert(file_type, editor.id());
        }

        self.editors.insert(editor.id(), editor);
    }

    /// Adds a source to this `DeltosEditor`, replacing any source with the same ID.
    ///
    /// Can be called at any time: once the UI is built, the file tree is rebuilt on the next
    /// frame to show the new source.
    ///
    /// # Parameters
    /// * source: The source to add.
    pub fn add_source(&mut self, source: Box<dyn Source>) {
        self.sources.insert(source.id(), Arc::from(source));
        self.request_rebuild(FileSelectorSideBar.default_slot());
    }

    /// Removes a source and closes every tab showing one of its files, discarding their
    /// unsaved changes.
    ///
    /// The file tree and editor area are rebuilt on the next frame.  Returns the removed
    /// source, or `None` if no source has that ID.
    ///
    /// # Parameters
    /// * id: ID of the source to remove.
    pub fn remove_source(&mut self, id: Uuid) -> Option<Arc<dyn Source>> {
        let removed = self.sources.remove(&id)?;

        // close the source's tabs, dropping tiles they leave empty
        let tile_ids: Vec<Uuid> = self.layout.tiles();
        for tile_id in tile_ids {
            let Some(tile) = self.tiles.get_mut(&tile_id) else { continue };
            let before = tile.tabs.len();
            tile.tabs.retain(|_, tab| tab.source != id);
            if tile.tabs.len() == before { continue }

            if tile.editor_tab.is_some_and(|tab| !tile.tabs.contains_key(&tab)) {
                tile.editor_tab = tile.tabs.keys().next().copied();
            }
            if tile.tabs.is_empty() {
                self.remove_tile(tile_id);
            }
            self.tiles_dirty = true;
        }

        self.request_rebuild(FileSelectorSideBar.default_slot());
        Some(removed)
    }

    /// Rebuilds the element shown in a side bar on the next frame, like the file tree after
    /// the host changed what a source lists.
    ///
    /// Does nothing before the editor's UI is built, which builds every side bar anyway.
    ///
    /// # Parameters
    /// * slot: The side bar to rebuild.
    pub fn request_rebuild(&mut self, slot: SideBarSlot) {
        self.side_bar_rebuilds.push(slot);
    }

    /// Shows a message to the user as a toast, and logs it at the matching level.
    ///
    /// Meant for failures the user should know about, like a source being unreachable, a
    /// save being refused or a file that changed remotely.
    ///
    /// # Parameters
    /// * level: How important the message is, which picks its color.
    /// * message: Text of the message.
    pub fn notify(&mut self, level: NotifyLevel, message: impl Into<String>) {
        let message = message.into();
        match level {
            NotifyLevel::Info => log::info!("{message}"),
            NotifyLevel::Warning => log::warn!("{message}"),
            NotifyLevel::Error => log::error!("{message}"),
        }
        self.notifications.push(Notification { level, message });
    }

    /// Add a top bar to this `DeltosEditor`.
    pub fn add_top_bar(&mut self, top_bar: Box<dyn TopBarElement>) {
        self.top_bars.insert(top_bar.id(), top_bar);
    }

    /// Add a top bar to this `DeltosEditor`.
    pub fn add_side_bar(&mut self, side_bar: Box<dyn SideBarElement>) {
        self.side_bars.insert(side_bar.id(), side_bar);
    }

    /// Opens a file in a tab, or focuses its tab if it is already open.
    ///
    /// New tabs open in the focused tile, falling back to the first tile in the layout.
    ///
    /// # Parameters
    /// * world: World the editor's UI lives in.
    /// * source: ID of the `Source` to load the file from.
    /// * path: Path of the file inside `source`.
    pub fn open_tab(&mut self, world: &World, source: Uuid, path: &PathBuf) {
        let file_id = tab_id(source, path);

        let file_name = path.file_name()
            .and_then(|str| str.to_str())
            .map(|str| str.to_string())
            .unwrap_or_default();

        let editor = path.extension()
            .and_then(|str| str.to_str())
            .and_then(|str| self.file_type_editor_lookup.get(str))
            .copied()
            .or(self.default_editor);

        // focus the tab instead if the file is already open in some tile
        let open_in = self.tiles.iter()
            .find(|(_, tile)| tile.tabs.contains_key(&file_id))
            .map(|(tile_id, _)| *tile_id);
        if let Some(tile_id) = open_in {
            self.focus_tab(world, tile_id, file_id);
            return;
        }

        // add the tab to the focused tile
        let tile_id = self.target_tile();
        let Some(tile) = self.tiles.get_mut(&tile_id) else { return };
        tile.tabs.insert(file_id, EditorTab { id: file_id, name: file_name, source, path: path.clone(), editor });
        self.rebuild_containers(world, tile_id, Some(file_id));
        self.rebuild_tab_strips(world, tile_id);
    }

    /// Shows a tab's editor in its tile and makes that tile the focused one.
    ///
    /// # Parameters
    /// * world: World the editor's UI lives in.
    /// * tile_id: ID of the tile holding the tab.
    /// * tab_id: ID of the tab to focus.
    pub fn focus_tab(&mut self, world: &World, tile_id: Uuid, tab_id: Uuid) {
        let Some(tile) = self.tiles.get_mut(&tile_id) else { return };
        if !tile.tabs.contains_key(&tab_id) { return }

        tile.editor_tab = None;
        self.focused_tile = tile_id;

        self.rebuild_containers(world, tile_id, Some(tab_id));
        self.rebuild_tab_strips(world, tile_id);
    }

    /// Closes a tab.  A tile left without tabs is removed, unless it is the last tile.
    ///
    /// # Parameters
    /// * world: World the editor's UI lives in.
    /// * tile_id: ID of the tile holding the tab.
    /// * tab_id: ID of the tab to close.
    pub fn close_tab(&mut self, world: &World, tile_id: Uuid, tab_id: Uuid) {
        let Some(tile) = self.tiles.get_mut(&tile_id) else { return };

        tile.tabs.remove(&tab_id);
        tile.editor_tab = None;

        // drop the tile from the layout once its last tab is gone
        if tile.tabs.is_empty() && self.remove_tile(tile_id) {
            self.rebuild_tiles(world);
            return;
        }

        let next_tab = self.tiles
            .get(&tile_id)
            .and_then(|tile| tile.tabs.keys().next().copied());
        self.rebuild_containers(world, tile_id, next_tab);
        self.rebuild_tab_strips(world, tile_id);
    }

    /// Moves a tab into another tile, or into a new tile split off beside one, then rebuilds
    /// the editor area.
    ///
    /// Dropping in `DropZone::Center` adds the tab to `to`'s tabs.  Any other zone splits `to`
    /// and puts the tab in a new tile on that side.  The tab keeps its editor, so unsaved edits
    /// move with it.  A tile left without tabs is removed.
    ///
    /// # Parameters
    /// * world: World the editor's UI lives in.
    /// * from: ID of the tile the tab is in.
    /// * tab_id: ID of the tab to move.
    /// * to: ID of the tile the tab is dropped on.
    /// * zone: Where on `to` the tab is dropped.
    pub fn move_tab(&mut self, world: &World, from: Uuid, tab_id: Uuid, to: Uuid, zone: DropZone) {
        if self.place_tab(from, tab_id, to, zone).is_some() {
            self.rebuild_tiles(world);
        }
    }

    /// Moves a tab between tiles without touching the UI, see `move_tab`.
    ///
    /// Returns the ID of the tile the tab ends up in, or `None` if nothing changed: the tab or
    /// a tile is missing, the tab is dropped back into the middle of its own tile, or the tab is
    /// the only one in its tile and dropped on that tile's edge.
    ///
    /// # Parameters
    /// * from: ID of the tile the tab is in.
    /// * tab_id: ID of the tab to move.
    /// * to: ID of the tile the tab is dropped on.
    /// * zone: Where on `to` the tab is dropped.
    pub fn place_tab(&mut self, from: Uuid, tab_id: Uuid, to: Uuid, zone: DropZone) -> Option<Uuid> {
        // refuse moves that would leave the layout as it is
        let source_tabs = self.tiles.get(&from)?.tabs.len();
        if !self.tiles.contains_key(&to) { return None }
        if from == to && (zone == DropZone::Center || source_tabs <= 1) { return None }

        // take the tab out of its tile, showing another tab there if it was shown
        let source = self.tiles.get_mut(&from)?;
        let tab = source.tabs.remove(&tab_id)?;
        if source.editor_tab == Some(tab_id) {
            source.editor_tab = source.tabs.keys().next().copied();
        }

        // pick the tile the tab goes into, splitting a new one off the target for edge drops
        let dest = if zone == DropZone::Center {
            to
        } else {
            let tile = EditorTile { id: Uuid::new_v4(), tabs: AHashMap::default(), editor_tab: None };
            if !self.layout.split(to, tile.id, zone) {
                // put the tab back so a failed split loses nothing
                if let Some(source) = self.tiles.get_mut(&from) { source.tabs.insert(tab_id, tab); }
                return None;
            }
            let id = tile.id;
            self.tiles.insert(id, tile);
            id
        };

        // add the tab to its new tile and show it there
        let tile = self.tiles.get_mut(&dest)?;
        tile.tabs.insert(tab_id, tab);
        tile.editor_tab = Some(tab_id);
        self.focused_tile = dest;

        if self.tiles.get(&from).is_some_and(|tile| tile.tabs.is_empty()) {
            self.remove_tile(from);
        }
        Some(dest)
    }

    /// Removes a tile from the layout and the tile list, moving focus off it if needed.
    /// Returns whether it was removed; the last tile is never removed.
    fn remove_tile(&mut self, tile_id: Uuid) -> bool {
        if !self.layout.remove(tile_id) { return false }
        self.tiles.remove(&tile_id);
        if self.focused_tile == tile_id {
            self.focused_tile = self.target_tile();
        }
        true
    }

    /// The tile new tabs open in: the focused tile, or the first tile if that one is gone.
    fn target_tile(&self) -> Uuid {
        if self.tiles.contains_key(&self.focused_tile) { return self.focused_tile }
        self.layout
            .tiles()
            .into_iter()
            .find(|id| self.tiles.contains_key(id))
            .unwrap_or(self.focused_tile)
    }

    /// Replaces the tabs in a tile's tab strip with ones built from the tile's tabs.
    fn rebuild_tab_strips(&mut self, world: &World, tile_id: Uuid) {
        let Some(tile) = self.tiles.get(&tile_id) else { return };

        // find the strip of this tile
        let Some((strip_id, _)) = Query::<&TabStrip>::new(world).iter()
            .find(|(_, strip)| strip.tile_id == tile_id)
            else { return };

        // wipe tab children of each strip (assuming all children are tabs)
        let old: Vec<EntityID> = world.hierarchy()
            .children(strip_id)
            .map(|c| c.to_vec())
            .unwrap_or_default();
        for child in old {
            world.handle(child).despawn_recursive();
        }

        // spawn a new tab for each tab entry stored in the editor
        for tab in tile.tabs.values() {
            world.spawn_with_parent(Some(strip_id), render::tab(self, tile, tab).into_entity_builder());
        }
    }

    /// Replaces the editor in a tile's container with the editor of `tab`, unless the tile's
    /// shown tab is still open.
    fn rebuild_containers(&mut self, world: &World, tile_id: Uuid, tab: Option<Uuid>) {
        let Some(tile) = self.tiles.get_mut(&tile_id) else { return };

        // query container
        let Some((entity_id, _)) = Query::<&mut EditorContainer>::new(world)
            .iter()
            .find(|(_, container)| container.tile_id == tile_id)
            else { return };

        // if tab mentioned in container is still active, skip container
        if tile.editor_tab.is_some_and(|tab| tile.tabs.contains_key(&tab)) {
            return;
        }

        // update tab
        tile.editor_tab = tab;

        // despawn old children
        let old: Vec<EntityID> = world.hierarchy()
            .children(entity_id)
            .map(|c| c.to_vec())
            .unwrap_or_default();
        for child in old {
            world.handle(child).despawn_recursive();
        }

        // build, spawn and append new editor
        let Some(bundle) = self.build_tab_editor(tile_id) else { return };
        world.spawn_with_parent(Some(entity_id), bundle.into_entity_builder());
    }

    /// Builds a placeholder for the editor of the tab shown in a tile, tagged with the tab it
    /// belongs to, and builds the editor in the background to replace it.
    fn build_tab_editor(&mut self, tile_id: Uuid) -> Option<UiBundle> {
        let tile = self.tiles.get(&tile_id)?;
        let tab = tile.tabs.get(&tile.editor_tab?)?;
        let editor_id = tab.path.extension()
            .map(|ext| ext.to_string_lossy().to_string())
            .and_then(|ext| self.file_type_editor_lookup.get(&ext))
            .or(self.default_editor.as_ref())?;
        let editor = self.editors.get_mut(editor_id)?;
        let source = self.sources.get(&tab.source)?.clone();
        let read_only = source.read_only();
        let build = editor.build_editor(&self.theme, source, tab.path.clone(), read_only);

        // swap the built editor in for the placeholder once it is ready
        let (tab_id, placeholder) = (tab.id, reserve_id());
        self.tasks.spawn(Box::pin(async move {
            let bundle = build.await.with(EditorContent { tab_id });
            let apply: TaskResult = Box::new(move |world: &World, _: &mut DeltosEditor| {
                replace_placeholder(world, placeholder, [bundle]);
            });
            apply
        }));

        // the placeholder carries the tab too, so it moves between tiles like a built editor
        let loading = UiBundle::new(Node::new().padding(Rect::single(px(12.0))))
            .with_id(placeholder)
            .with(EditorContent { tab_id })
            .text(Text::new(self.font.clone(), "Loading...", self.theme.bg_5, 14.0));
        Some(loading)
    }

    /// Rebuilds the whole editor area from `layout`, after tiles were added, removed or moved.
    ///
    /// Shown editors are kept and moved into the container of whichever tile now shows their
    /// tab, so their state (like unsaved text) survives.  Editors no longer shown are dropped.
    fn rebuild_tiles(&mut self, world: &World) {
        let Some((area, _)) = Query::<&TileArea>::new(world).iter().next() else { return };

        // unlink the shown editors so they outlive the old tiles
        let mut kept: AHashMap<Uuid, EntityID> = Query::<&EditorContent>::new(world)
            .iter()
            .map(|(entity, content)| (content.tab_id, entity))
            .collect();
        for entity in kept.values() {
            world.hierarchy().remove_parent(*entity);
        }

        // replace the old tiles with ones built from the layout
        let old: Vec<EntityID> = world.hierarchy()
            .children(area)
            .map(|c| c.to_vec())
            .unwrap_or_default();
        for child in old {
            world.handle(child).despawn_recursive();
        }
        world.spawn_with_parent(Some(area), render::tile_layout(self, &self.layout).into_entity_builder());

        // fill each new container with its tab's kept editor, or a freshly built one
        let containers: Vec<(EntityID, Uuid)> = Query::<&EditorContainer>::new(world)
            .iter()
            .map(|(entity, container)| (entity, container.tile_id))
            .collect();
        for (container, tile_id) in containers {
            let shown = self.tiles
                .get(&tile_id)
                .and_then(|tile| tile.editor_tab);
            if let Some(entity) = shown.and_then(|tab| kept.remove(&tab)) {
                if let Err(err) = world.hierarchy().add_child(container, entity) {
                    log::error!("Failed to move an editor into its new tile: {err:#}");
                }
            } else if let Some(bundle) = self.build_tab_editor(tile_id) {
                world.spawn_with_parent(Some(container), bundle.into_entity_builder());
            }
        }

        // drop editors whose tab is no longer shown anywhere
        for entity in kept.into_values() {
            world.handle(entity).despawn_recursive();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::memory::MemorySource;

    /// Builds an editor with the bundled font and no sources or editors.
    fn editor() -> DeltosEditor {
        let font = SDFFont::new(include_bytes!("../fonts/BlexMonoNerdFontMono-Regular.ttf"))
            .expect("bundled font loads");
        DeltosEditor::new(Arc::new(font))
    }

    /// Adds a tab for `name` to `tile` and shows it, returning the tab's ID.
    fn add_tab(editor: &mut DeltosEditor, tile: Uuid, name: &str) -> Uuid {
        let id = Uuid::new_v4();
        let tab = EditorTab { id, name: name.into(), source: Uuid::nil(), path: PathBuf::from(name), editor: None };
        let tile = editor.tiles.get_mut(&tile).expect("tile exists");
        tile.tabs.insert(id, tab);
        tile.editor_tab = Some(id);
        id
    }

    #[test]
    fn edge_drop_splits_into_new_tile() {
        let mut editor = editor();
        let first = editor.focused_tile;
        let (a, b) = (add_tab(&mut editor, first, "a.txt"), add_tab(&mut editor, first, "b.txt"));

        let dest = editor.place_tab(first, b, first, DropZone::Right).expect("tab moved");

        assert_ne!(dest, first);
        assert_eq!(editor.layout.tiles(), vec![first, dest]);
        assert_eq!(editor.focused_tile, dest);
        assert!(editor.tiles[&dest].tabs.contains_key(&b));
        assert_eq!(editor.tiles[&dest].editor_tab, Some(b));
        assert!(!editor.tiles[&first].tabs.contains_key(&b));
        assert_eq!(editor.tiles[&first].editor_tab, Some(a));
    }

    #[test]
    fn center_drop_joins_tile_and_removes_empty_source() {
        let mut editor = editor();
        let first = editor.focused_tile;
        add_tab(&mut editor, first, "a.txt");
        let b = add_tab(&mut editor, first, "b.txt");
        let second = editor.place_tab(first, b, first, DropZone::Bottom).expect("split made");

        let dest = editor.place_tab(second, b, first, DropZone::Center).expect("tab moved back");

        assert_eq!(dest, first);
        assert_eq!(editor.layout, TileLayout::Tile(first));
        assert!(!editor.tiles.contains_key(&second));
        assert_eq!(editor.tiles[&first].tabs.len(), 2);
        assert_eq!(editor.tiles[&first].editor_tab, Some(b));
    }

    #[test]
    fn dropping_only_tab_on_its_own_tile_does_nothing() {
        let mut editor = editor();
        let first = editor.focused_tile;
        let a = add_tab(&mut editor, first, "a.txt");

        for zone in [DropZone::Center, DropZone::Left, DropZone::Top] {
            assert_eq!(editor.place_tab(first, a, first, zone), None, "{zone:?} moved the tab");
        }
        assert_eq!(editor.layout, TileLayout::Tile(first));
        assert!(editor.tiles[&first].tabs.contains_key(&a));
    }

    #[test]
    fn moving_only_tab_to_other_tile_edge_replaces_its_tile() {
        let mut editor = editor();
        let first = editor.focused_tile;
        add_tab(&mut editor, first, "a.txt");
        let b = add_tab(&mut editor, first, "b.txt");
        let second = editor.place_tab(first, b, first, DropZone::Right).expect("split made");

        // the moved tab's old tile empties and collapses, leaving the new split
        let third = editor.place_tab(second, b, first, DropZone::Top).expect("tab moved");

        assert_eq!(editor.layout.tiles(), vec![third, first]);
        assert!(!editor.tiles.contains_key(&second));
        assert!(matches!(editor.layout, TileLayout::Split { axis: tiles::SplitAxis::Vertical, .. }));
    }

    #[test]
    fn unknown_tabs_and_tiles_are_refused() {
        let mut editor = editor();
        let first = editor.focused_tile;
        let a = add_tab(&mut editor, first, "a.txt");
        add_tab(&mut editor, first, "b.txt");

        assert_eq!(editor.place_tab(first, Uuid::new_v4(), first, DropZone::Left), None);
        assert_eq!(editor.place_tab(first, a, Uuid::new_v4(), DropZone::Center), None);
        assert_eq!(editor.place_tab(Uuid::new_v4(), a, first, DropZone::Center), None);
        assert_eq!(editor.layout, TileLayout::Tile(first));
        assert_eq!(editor.tiles[&first].tabs.len(), 2);
    }

    #[test]
    fn removing_focused_tile_moves_focus() {
        let mut editor = editor();
        let first = editor.focused_tile;
        add_tab(&mut editor, first, "a.txt");
        let b = add_tab(&mut editor, first, "b.txt");
        let second = editor.place_tab(first, b, first, DropZone::Left).expect("split made");
        assert_eq!(editor.focused_tile, second);

        assert!(editor.remove_tile(second));
        assert_eq!(editor.focused_tile, first);
        assert!(!editor.remove_tile(first), "the last tile was removed");
    }

    /// Every tab ID open in any tile.
    fn open_tabs(editor: &DeltosEditor) -> Vec<Uuid> {
        editor.tiles
            .values()
            .flat_map(|tile| tile.tabs.keys().copied())
            .collect()
    }

    #[test]
    fn equal_paths_in_different_sources_get_their_own_tabs() {
        let mut editor = editor();
        let world = World::default();
        let (main, foo) = (MemorySource::new("main"), MemorySource::new("foo"));
        let path = PathBuf::from("./Cargo.toml");
        editor.add_source(Box::new(main.clone()));
        editor.add_source(Box::new(foo.clone()));

        editor.open_tab(&world, main.id(), &path);
        editor.open_tab(&world, foo.id(), &path);
        editor.open_tab(&world, main.id(), &path);

        let mut tabs = open_tabs(&editor);
        tabs.sort();
        let mut expected = vec![tab_id(main.id(), &path), tab_id(foo.id(), &path)];
        expected.sort();
        assert_eq!(tabs, expected, "reopening a file focuses its tab instead of adding one");
        assert_ne!(tab_id(main.id(), &path), tab_id(foo.id(), &path));
    }

    #[test]
    fn roots_sort_by_order_then_name() {
        let mut editor = editor();
        editor.add_source(Box::new(MemorySource::new("gearbox").with_order(2).with_read_only(true)));
        editor.add_source(Box::new(MemorySource::new("zeta").with_order(1)));
        editor.add_source(Box::new(MemorySource::new("alpha").with_order(1)));
        editor.add_source(Box::new(MemorySource::new("main").with_order(0)));

        let names: Vec<String> = side_bar::file_selector::sorted_sources(&editor)
            .iter()
            .map(|source| source.name())
            .collect();

        assert_eq!(names, ["main", "alpha", "zeta", "gearbox"]);
    }

    #[test]
    fn removing_a_source_closes_its_tabs() {
        let mut editor = editor();
        let world = World::default();
        let (main, foo) = (MemorySource::new("main"), MemorySource::new("foo"));
        editor.add_source(Box::new(main.clone()));
        editor.add_source(Box::new(foo.clone()));
        let first = editor.focused_tile;
        editor.open_tab(&world, main.id(), &PathBuf::from("a.txt"));
        editor.open_tab(&world, foo.id(), &PathBuf::from("b.txt"));
        let foo_tab = tab_id(foo.id(), Path::new("b.txt"));
        editor.place_tab(first, foo_tab, first, DropZone::Right).expect("split made");
        editor.tiles_dirty = false;

        assert!(editor.remove_source(foo.id()).is_some());

        assert!(!editor.sources.contains_key(&foo.id()));
        assert_eq!(open_tabs(&editor), vec![tab_id(main.id(), Path::new("a.txt"))]);
        assert_eq!(editor.layout, TileLayout::Tile(first), "the emptied tile is removed");
        assert!(editor.tiles_dirty);
        assert!(editor.remove_source(foo.id()).is_none());
    }
}
