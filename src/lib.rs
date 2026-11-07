use std::fmt::{Debug, Formatter};
use std::path::PathBuf;
use std::sync::Arc;

use anarchy::*;
use gearbox::egui::egui::ahash::AHashMap;
use gearbox::{App, GearboxRenderPlugin, Plugin};
use gearbox::ui::*;
use uuid::{Uuid, uuid};

use crate::editors::FileEditor;
use crate::sources::local::LocalFileSource;
use crate::window::{EditorContainer, TabStrip, WindowPlugin};

pub const EDITOR_UUID_NAMESPACE: Uuid = uuid!("dcf01b57-1883-4daf-a843-49e6a0bfdc27");
pub const TAB_UUID_NAMESPACE: Uuid = uuid!("fefe4e8b-7e6b-4e1b-a728-249b6b052d71");

pub mod colors;
pub mod editors;
pub mod render;
pub mod sources;
pub mod window;

/// The primary plugin for `Deltos` editors.  This initializes all core
/// requirements for the `DeltosEditor`.
pub struct DeltosPlugin { font: Arc<SDFFont> }

impl DeltosPlugin {
    /// Create a new `DeltosPlugin` that uses the given font
    /// for the editor.  If you do not have a font to provide
    /// use the `default` function.
    pub fn new(font: SDFFont) -> Self {
        Self { font: Arc::new(font) }
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
        app .add_plugin(GearboxRenderPlugin)
            .add_plugin(UIPlugin)
            .add_plugin(WidgetPlugin::new(self.font.clone()))
            .add_plugin(WindowPlugin)
            .add_resource(DeltosEditor::new(self.font.clone()))
    }
}

/// The primary storage for the Detlos Editor.
#[derive(Resource, Debug)]
pub struct DeltosEditor {
    pub font: Arc<SDFFont>,
    pub tabs: AHashMap<Uuid, EditorTab>,
    pub editors: AHashMap<Uuid, Box<dyn FileEditor>>,
    pub file_type_editor_lookup: AHashMap<String, Uuid>,
    pub default_editor: Option<Uuid>
}

/// A container of all information of an active editor.
/// Brings together a tabs information and there parent `FileEditor`.
pub struct EditorTab {
    pub id: Uuid,
    pub name: String,
    pub path: PathBuf,
    pub editor: Option<Uuid>
}

impl Debug for EditorTab {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditorTab")
            .field("id", &self.id)
            .field("name", &self.name)
            .finish()
    }
}

impl DeltosEditor {
    pub fn new(font: Arc<SDFFont>) -> Self {
        Self {
            font,
            tabs: AHashMap::default(),
            editors: AHashMap::default(),
            file_type_editor_lookup: AHashMap::default(),
            default_editor: None
        }
    }

    pub fn add_editor(&mut self, editor: Box<dyn FileEditor>, can_be_default: bool) {
        if self.default_editor.is_none() && can_be_default {
            self.default_editor = Some(editor.id());
        }

        for file_type in editor.file_types() {
            self.file_type_editor_lookup.insert(file_type, editor.id());
        }

        self.editors.insert(editor.id(), editor);
    }

    pub fn open_tab(&mut self, world: &World, path: &PathBuf) {
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

        if self.tabs.contains_key(&file_id) {
            println!("TODO focus tab {file_id}");
            return;
        }

        self.tabs.insert(file_id, EditorTab { id: file_id, name: file_name, path: path.clone(), editor });

        self.rebuild_tab_strips(world);
        self.rebuild_containers(world, Some(file_id));
    }

    pub fn focus_tab(&mut self, world: &World, tab_id: Uuid) {
        if !self.tabs.contains_key(&tab_id) { return }

        let containers: Vec<(EntityID, Mut<EditorContainer>)> = {
            let mut query = Query::<&mut EditorContainer>::new(world);
            let mut containers = Vec::new();
            while let Ok(Some((id, container))) = query.next() { containers.push((id, container)); }
            containers
        };

        for (_, mut container) in containers {
            container.editor_tab = None;
        }

        self.rebuild_tab_strips(world);
        self.rebuild_containers(world, Some(tab_id));
    }

    pub fn close_tab(&mut self, world: &World, tab_id: Uuid) {
        self.tabs.remove(&tab_id);

        let next_tab = self.tabs.keys().next().copied();

        let containers: Vec<(EntityID, Mut<EditorContainer>)> = {
            let mut query = Query::<&mut EditorContainer>::new(world);
            let mut containers = Vec::new();
            while let Ok(Some((id, container))) = query.next() { containers.push((id, container)); }
            containers
        };

        for (_, mut container) in containers {
            container.editor_tab = None;
        }

        self.rebuild_tab_strips(world);
        self.rebuild_containers(world, next_tab);
    }

    fn rebuild_tab_strips(&mut self, world: &World) {
        // query all active tab strips and editor containers before iterating for replacement
        let strips: Vec<EntityID> = {
            let mut query = Query::<&TabStrip>::new(world);
            let mut ids = Vec::new();
            while let Ok(Some((id, _))) = query.next() { ids.push(id); }
            ids
        };

        for strip in strips {
            // wipe tab children of each strip (assuming all children are tabs)
            let old: Vec<EntityID> = world.hierarchy()
                .children(strip)
                .map(|c| c.to_vec())
                .unwrap_or_default();
            for child in old {
                world.handle(child).despawn_recursive();
            }

            // spawn a new tab for each tab entry stored in the editor
            for tab in self.tabs.values() {
                world.spawn_with_parent(Some(strip), render::tab(self, tab).into_entity_builder());
            }
        }
    }

    fn rebuild_containers(&mut self, world: &World, tab: Option<Uuid>) {
        let containers: Vec<(EntityID, Mut<EditorContainer>)> = {
            let mut query = Query::<&mut EditorContainer>::new(world);
            let mut containers = Vec::new();
            while let Ok(Some((id, container))) = query.next() { containers.push((id, container)); }
            containers
        };

        for (entity_id, mut container) in containers {
            // if tab mentioned in container is still active, skip container
            if container.editor_tab.is_some() && self.tabs.contains_key(container.editor_tab.as_ref().unwrap()) {
                continue;
            }

            // update tab
            container.editor_tab = tab;

            // despawn old children
            let old: Vec<EntityID> = world.hierarchy()
                .children(entity_id)
                .map(|c| c.to_vec())
                .unwrap_or_default();
            for child in old {
                world.handle(child).despawn_recursive();
            }

            // build and append new child
            let Some(tab) = container.editor_tab
                .map(|tab| self.tabs.get(&tab))
                .flatten()
                else { continue };
            let Some(editor_id) = tab.path.extension()
                .map(|ext| ext.to_string_lossy().to_string())
                .map(|ext| self.file_type_editor_lookup.get(&ext))
                .flatten()
                .or_else(|| self.default_editor.as_ref())
                else { continue };
            let Some(editor) = self.editors.get_mut(editor_id)
                else { continue };

            // build, spawn and append new editor
            let bundle = editor.build_editor(&LocalFileSource, tab.path.clone());
            let child = world.spawn(bundle.into_entity_builder());
            let _ = world.hierarchy().add_child(entity_id, child);
        }
    }
}
