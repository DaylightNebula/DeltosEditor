use std::fmt::{Debug, Formatter};
use std::path::PathBuf;
use std::sync::Arc;

use anarchy::*;
use gearbox::egui::egui::ahash::AHashMap;
use gearbox::{App, GearboxRenderPlugin, Plugin};
use gearbox::ui::*;
use uuid::{Uuid, uuid};

use crate::editors::FileEditor;
use crate::hover::HoverPlugin;
use crate::sources::Source;
use crate::theme::Theme;
use crate::top_bar::TopBarElement;
use crate::top_bar::title_decor::TitleDecorElement;
use crate::top_bar::window_decor::TopWindowDecorElement;
use crate::window::{EditorContainer, TabStrip, WindowPlugin};

pub const TOP_BAR_UUID_NAMESPACE: Uuid = uuid!("db204c32-f0fd-4b55-991a-6fd4ed77f102");
pub const SOURCE_UUID_NAMESPACE: Uuid = uuid!("c007d5db-1320-45a4-9f70-163706d055ea");
pub const EDITOR_UUID_NAMESPACE: Uuid = uuid!("dcf01b57-1883-4daf-a843-49e6a0bfdc27");
pub const TAB_UUID_NAMESPACE: Uuid = uuid!("fefe4e8b-7e6b-4e1b-a728-249b6b052d71");

pub mod editors;
pub mod hover;
pub mod render;
pub mod sources;
pub mod theme;
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
            .add_resource(editor)
            .on_startup(|mut editor: ResMut<DeltosEditor>| {
                editor.add_top_bar(Box::new(TitleDecorElement));
                editor.add_top_bar(Box::new(TopWindowDecorElement));
            })
    }
}

/// The primary storage for the Detlos Editor.
#[derive(Resource, Debug)]
pub struct DeltosEditor {
    pub title: String,
    pub font: Arc<SDFFont>,
    pub theme: Theme,

    pub sources: AHashMap<Uuid, Box<dyn Source>>,
    pub top_bars: AHashMap<Uuid, Box<dyn TopBarElement>>,
    pub editors: AHashMap<Uuid, Box<dyn FileEditor>>,

    pub tiles: AHashMap<Uuid, EditorTile>,
    pub file_type_editor_lookup: AHashMap<String, Uuid>,
    pub default_editor: Option<Uuid>
}

/// A container of tabs that can be built into one tile.
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
        let mut tiles = AHashMap::default();
        tiles.insert(default_tile.id, default_tile);

        Self {
            font, tiles,
            title: "Deltos Editor".to_string(),
            theme: Theme::default(),
            sources: AHashMap::default(),
            editors: AHashMap::default(),
            top_bars: AHashMap::default(),
            file_type_editor_lookup: AHashMap::default(),
            default_editor: None
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

    /// Add a source to this `DeltosEditor`.
    pub fn add_source(&mut self, source: Box<dyn Source>) {
        self.sources.insert(source.id(), source);
    }

    /// Add a top bar to this `DeltosEditor`.
    pub fn add_top_bar(&mut self, top_bar: Box<dyn TopBarElement>) {
        self.top_bars.insert(top_bar.id(), top_bar);
    }

    pub fn open_tab(&mut self, world: &World, source: Uuid, path: &PathBuf) {
        let file_id = Uuid::new_v5(&TAB_UUID_NAMESPACE, path.as_os_str().as_encoded_bytes());

        let file_name = path.file_name()
            .map(|str| str.to_str())
            .flatten()
            .map(|str| str.to_string())
            .unwrap_or_else(|| String::new());

        let editor = path.extension()
            .map(|str| str.to_str())
            .flatten()
            .map(|str| self.file_type_editor_lookup.get(str))
            .flatten()
            .copied()
            .or_else(|| self.default_editor);

        let open_in = self.tiles.iter()
            .find(|(_, tile)| tile.tabs.contains_key(&file_id))
            .map(|(tile_id, _)| *tile_id);
        if let Some(tile_id) = open_in {
            self.focus_tab(world, tile_id, file_id);
            return;
        }

        let tile_id = {
            let Some((tile_id, tile)) = self.tiles.iter_mut().next() else { return };
            tile.tabs.insert(file_id, EditorTab { id: file_id, name: file_name, source, path: path.clone(), editor });
            *tile_id
        };
        self.rebuild_containers(world, tile_id, Some(file_id));
        self.rebuild_tab_strips(world, tile_id);
    }

    pub fn focus_tab(&mut self, world: &World, tile_id: Uuid, tab_id: Uuid) {
        let Some(tile) = self.tiles.get_mut(&tile_id) else { return };
        if !tile.tabs.contains_key(&tab_id) { return }

        tile.editor_tab = None;

        self.rebuild_containers(world, tile_id, Some(tab_id));
        self.rebuild_tab_strips(world, tile_id);
    }

    pub fn close_tab(&mut self, world: &World, tile_id: Uuid, tab_id: Uuid) {
        let Some(tile) = self.tiles.get_mut(&tile_id) else { return };

        tile.tabs.remove(&tab_id);
        tile.editor_tab = None;

        let next_tab = tile.tabs.keys().next().copied();
        self.rebuild_containers(world, tile_id, next_tab);
        self.rebuild_tab_strips(world, tile_id);
    }

    fn rebuild_tab_strips(&mut self, world: &World, tile_id: Uuid) {
        let Some(tile) = self.tiles.get(&tile_id) else { return };

        // query all active tab strips and editor containers before iterating for replacement
        let Some((strip_id, _)) = Query::<&TabStrip>::new(world).iter()
            .filter(|(_, strip)| strip.tile_id == tile_id)
            .next()
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

    fn rebuild_containers(&mut self, world: &World, tile_id: Uuid, tab: Option<Uuid>) {
        let Some(tile) = self.tiles.get_mut(&tile_id) else { return };

        // query container
        let Some((entity_id, _)) = Query::<&mut EditorContainer>::new(world)
            .iter()
            .filter(|(_, container)| container.tile_id == tile_id)
            .next()
            else { return };

        // if tab mentioned in container is still active, skip container
        if tile.editor_tab.is_some() && tile.tabs.contains_key(tile.editor_tab.as_ref().unwrap()) {
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

        // build and append new child
        let Some(tab) = tile.editor_tab
            .map(|tab| tile.tabs.get(&tab))
            .flatten()
            else { return };
        let Some(editor_id) = tab.path.extension()
            .map(|ext| ext.to_string_lossy().to_string())
            .map(|ext| self.file_type_editor_lookup.get(&ext))
            .flatten()
            .or_else(|| self.default_editor.as_ref())
            else { return };
        let Some(editor) = self.editors.get_mut(editor_id)
            else { return };
        let Some(source) = self.sources.get(&tab.source)
            else { return };
        let path = tab.path.clone();

        // build, spawn and append new editor
        let bundle = editor.build_editor(&self.theme, source.as_ref(), path);
        let child = world.spawn(bundle.into_entity_builder());
        let _ = world.hierarchy().add_child(entity_id, child);
    }
}
