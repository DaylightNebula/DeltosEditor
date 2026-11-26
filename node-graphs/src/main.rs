use deltos::{DeltosEditor, DeltosPlugin, editors::text::SimpleTextDeltosEditorPlugin, theme::Theme};
use deltos_node_graphs::{DeltosNodeGraphPlugin, IOButton, NodeGraphTile, editor::{NodeGraphEditor, NodeGraphFileEditor}};
use gearbox::{App, Plugin, anarchy::ResMut, anyhow, glam::{Vec2, Vec4}};
use uuid::Uuid;

/// Demo tile that builds a transform from a position and a scale.
///
/// Fields:
/// * tile_id: Unique ID of this tile.
/// * position: Where the tile is drawn, in logical pixels from the graph's top left.
#[derive(Debug)]
struct TransformTile {
    tile_id: Uuid,
    position: Vec2
}

impl NodeGraphTile for TransformTile {
    fn tile_id(&self) -> Uuid { self.tile_id }
    fn title(&self) -> String { "Transform".into() }
    fn color(&self, theme: &Theme) -> Vec4 { theme.orange }
    fn position(&self) -> Vec2 { self.position }

    fn set_position(&mut self, position: Vec2) {
        self.position = position;
    }

    fn inputs(&self, theme: &Theme) -> Vec<IOButton> {
        vec![
            IOButton::new("Position", theme.blue),
            IOButton::new("Scale", theme.green)
        ]
    }

    fn outputs(&self, theme: &Theme) -> Vec<IOButton> {
        vec![IOButton::new("Transform", theme.purple)]
    }
}

/// Demo node graph editor that shows a single tile for `.graph` files.
///
/// Fields:
/// * tiles: The tiles of the graph.
#[derive(Debug)]
struct DemoNodeGraphEditor {
    tiles: Vec<TransformTile>
}

impl NodeGraphEditor for DemoNodeGraphEditor {
    type Tile = TransformTile;

    fn name(&self) -> String { "Demo Node Graph".into() }

    fn file_types(&self) -> Vec<String> { vec!["graph".into()] }

    fn tiles(&self) -> &[TransformTile] { &self.tiles }

    fn tiles_mut(&mut self) -> &mut [TransformTile] { &mut self.tiles }
}

/// Registers the demo node graph editor with the Deltos editor.
struct DemoNodeGraphPlugin;

impl Plugin for DemoNodeGraphPlugin {
    fn build(self, app: App) -> App {
        app .add_plugin(DeltosNodeGraphPlugin)
            .on_startup(|mut editor: ResMut<DeltosEditor>| {
                let demo = DemoNodeGraphEditor {
                    tiles: vec![TransformTile { tile_id: Uuid::new_v4(), position: Vec2::new(40.0, 40.0) }]
                };
                let graph_editor = NodeGraphFileEditor::new(demo, editor.font.clone());
                editor.add_editor(Box::new(graph_editor), false);
            })
    }
}

/// Opens the editor on the local file system, which only exists natively.  On the web the
/// editor starts without sources; see the `web` example for one with sources.
fn main() -> anyhow::Result<()> {
    let plugin = DeltosPlugin::default();

    #[cfg(not(target_arch = "wasm32"))]
    let plugin = plugin.with_source(deltos::sources::local::LocalFileSource);

    App::new()
        .add_plugin(plugin)
        .add_plugin(SimpleTextDeltosEditorPlugin)
        .add_plugin(DemoNodeGraphPlugin)
        .run()
}
