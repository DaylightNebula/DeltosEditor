use deltos::{DeltosEditor, DeltosPlugin, editors::text::SimpleTextDeltosEditorPlugin, theme::Theme};
use deltos_node_graphs::{DeltosNodeGraphPlugin, editor::{NodeGraphEditor, NodeGraphFileEditor}, tiles::{IOButton, NodeGraphTile, Wire, WireRules, Wires}};
use gearbox::{App, Plugin, anarchy::ResMut, anyhow::{self, Context}, glam::{Vec2, Vec4}, ui::{Node, Rect, UiBundle, px, themed}};
use uuid::Uuid;

/// ID of every transform tile's "Position" input.  Wires also name the tile, so button IDs only
/// need to be unique within a tile.
const POSITION_INPUT: Uuid = Uuid::from_u128(1);

/// ID of every transform tile's "Scale" input.
const SCALE_INPUT: Uuid = Uuid::from_u128(2);

/// ID of every transform tile's "Transform" output.
const TRANSFORM_OUTPUT: Uuid = Uuid::from_u128(3);

/// Demo tile that builds a transform from a position and a scale.
///
/// Fields:
/// * tile_id: Unique ID of this tile.
/// * position: Where the tile is drawn, in logical pixels from the graph's top left.
/// * wires: The wires out of the "Transform" output.
#[derive(Debug)]
struct TransformTile {
    tile_id: Uuid,
    position: Vec2,
    wires: Vec<Wire>
}

impl TransformTile {
    /// Creates an unconnected transform tile at `position`.
    fn new(position: Vec2) -> Self {
        Self { tile_id: Uuid::new_v4(), position, wires: Vec::new() }
    }
}

impl NodeGraphTile for TransformTile {
    fn tile_id(&self) -> Uuid { self.tile_id }
    fn title(&self) -> String { "Transform".into() }
    fn color(&self, theme: &Theme) -> Vec4 { theme.orange }
    fn position(&self) -> Vec2 { self.position }

    fn set_position(&mut self, position: Vec2) {
        self.position = position;
    }

    fn connect(&mut self, output_button_id: Uuid, input_tile_id: Uuid, input_button_id: Uuid) -> anyhow::Result<()> {
        anyhow::ensure!(output_button_id == TRANSFORM_OUTPUT, "No output with ID {output_button_id}");
        anyhow::ensure!(input_tile_id != self.tile_id, "A tile can't connect to itself");
        self.wires.push(Wire::new(input_tile_id, input_button_id));
        Ok(())
    }

    fn disconnect(&mut self, output_button_id: Uuid, input_tile_id: Uuid, input_button_id: Uuid) -> anyhow::Result<()> {
        anyhow::ensure!(output_button_id == TRANSFORM_OUTPUT, "No output with ID {output_button_id}");
        let wire = Wire::new(input_tile_id, input_button_id);
        let index = self.wires
            .iter()
            .position(|existing| *existing == wire)
            .with_context(|| format!("Transform isn't connected to input {input_button_id}"))?;
        self.wires.remove(index);
        Ok(())
    }

    fn inputs(&self, theme: &Theme) -> Vec<IOButton> {
        vec![
            IOButton::new(POSITION_INPUT, "Position", theme.blue, WireRules::Single),
            IOButton::new(SCALE_INPUT, "Scale", theme.green, WireRules::Single)
        ]
    }

    fn outputs(&self, theme: &Theme) -> Vec<(IOButton, Wires)> {
        vec![(IOButton::new(TRANSFORM_OUTPUT, "Transform", theme.purple, WireRules::Multi), Wires::new(self.wires.clone()))]
    }
}

/// Demo node graph editor that opens every `.graph` file as three connectable tiles.  Demo graphs
/// can't be saved.
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

    fn open(_content: &str) -> anyhow::Result<Self> {
        Ok(Self {
            tiles: vec![
                TransformTile::new(Vec2::new(40.0, 40.0)),
                TransformTile::new(Vec2::new(320.0, 160.0)),
                TransformTile::new(Vec2::new(320.0, 340.0))
            ]
        })
    }

    fn save(&self) -> anyhow::Result<String> {
        anyhow::bail!("Demo graphs can't be saved")
    }

    fn new_tile_options(&self) -> Vec<String> { vec!["Transform".into()] }

    fn new_tile(&mut self, name: String, position: Vec2) {
        if name == "Transform" { self.tiles.push(TransformTile::new(position)) }
    }

    fn remove_tile(&mut self, tile_id: Uuid) -> anyhow::Result<()> {
        let index = self.tiles
            .iter()
            .position(|tile| tile.tile_id == tile_id)
            .with_context(|| format!("No tile {tile_id} in the graph"))?;
        self.tiles.remove(index);
        Ok(())
    }

    fn tiles(&self) -> &[TransformTile] { &self.tiles }

    fn tiles_mut(&mut self) -> &mut [TransformTile] { &mut self.tiles }

    fn overlay(&self) -> Option<UiBundle> {
        // a demo button pinned to the top right of the graph
        let button = themed::button("Overlay");
        let corner = UiBundle::new(Node::row().absolute(Rect::new_top_right(px(12.0), px(12.0))));
        Some(corner.child(button))
    }
}

/// Registers the demo node graph editor with the Deltos editor.
struct DemoNodeGraphPlugin;

impl Plugin for DemoNodeGraphPlugin {
    fn build(self, app: App) -> App {
        app .add_plugin(DeltosNodeGraphPlugin)
            .on_startup(|mut editor: ResMut<DeltosEditor>| {
                let graph_editor = NodeGraphFileEditor::new(DemoNodeGraphEditor { tiles: Vec::new() });
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
