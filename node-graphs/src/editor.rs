use std::{fmt::Debug, path::PathBuf, sync::{Arc, Mutex}};

use deltos::{editors::{FileEditor, text::OpenFile}, sources::{BoxFuture, Source}, theme::Theme};
use gearbox::{anyhow, glam::{Vec2, Vec4}, ui::*};
use uuid::Uuid;

use crate::{DeleteTileItem, GraphRoot, HoverRing, IODot, NewTileItem, PropertyField, SharedGraph, TileNode, WireNode, WirePreview, set_top_left, tiles::{IOButton, NodeGraphTile, TileProperty}};

const TITLE_SIZE: f32 = 18.0;
const IO_SIZE: f32 = 14.0;
const TILE_RADIUS: f32 = 8.0;
const TILE_BORDER: f32 = 1.5;
const IO_DOT: f32 = 10.0;
pub(crate) const RING_SIZE: f32 = 18.0;
const RING_WIDTH: f32 = 1.5;
pub(crate) const WIRE_THICKNESS: f32 = 3.0;
pub(crate) const WIRE_HOVER_THICKNESS: f32 = 4.5;
const WIRE_SAMPLES: usize = 32;
const WIRE_MIN_BEND: f32 = 40.0;
pub(crate) const WIRE_HIT_DISTANCE: f32 = 6.0;
const NEW_TILE_MENU_WIDTH: f32 = 180.0;
const TILE_MENU_WIDTH: f32 = 140.0;
const PROPERTY_FIELD_WIDTH: f32 = 90.0;

/// Implemented by anything that describes a node graph for a set of file types.  Wrapped in a
/// `NodeGraphFileEditor` so the Deltos editor can open files with it.  Every opened file gets its
/// own graph from `open`, which `save` writes back.
pub trait NodeGraphEditor: Debug + Send + 'static {
    /// The tile implementation.
    type Tile: NodeGraphTile;

    /// The display name of this editor.
    fn name(&self) -> String;

    /// A list of file types this editor works on.
    fn file_types(&self) -> Vec<String>;

    /// Builds the graph of an opened file from its text.  Called once per opened file, so each
    /// file is edited by its own graph.
    ///
    /// # Errors
    /// * `content` isn't a graph this editor can read.
    fn open(content: &str) -> anyhow::Result<Self> where Self: Sized;

    /// Writes the graph as the text of its file, to be read back by `open`.  Called when the
    /// file is saved.
    ///
    /// # Errors
    /// * The graph can't be written, such as a graph that can't be saved in its current state.
    fn save(&self) -> anyhow::Result<String>;

    /// List of options for new tiles.
    fn new_tile_options(&self) -> Vec<String>;

    /// Create a new tile at a given position using the given name
    /// supplied from the above new_tile_options.
    fn new_tile(&mut self, name: String, position: Vec2);

    /// Removes the tile with `tile_id` from the graph.  Wires into the tile are disconnected
    /// through `NodeGraphTile::disconnect` before this is called, and wires out of it go with it.
    ///
    /// # Errors
    /// * There's no tile with `tile_id`, or it can't be removed.
    fn remove_tile(&mut self, tile_id: Uuid) -> anyhow::Result<()>;

    /// The tiles of the graph to draw.
    fn tiles(&self) -> &[Self::Tile];

    /// The tiles of the graph, mutably.  Used to store a tile's new position when it is dragged.
    fn tiles_mut(&mut self) -> &mut [Self::Tile];
}

/// Adapts a `NodeGraphEditor` into a Deltos `FileEditor` that draws its tiles.
///
/// Fields:
/// * editor: Describes the editor's name and file types.  Opened files get their own graph from
///   `NodeGraphEditor::open` instead of sharing this one.
/// * font: Font used for tile titles and input/output names.
#[derive(Debug)]
pub struct NodeGraphFileEditor<Editor: NodeGraphEditor> {
    editor: Editor,
    font: Arc<SDFFont>
}

impl <Editor: NodeGraphEditor> NodeGraphFileEditor<Editor> {
    /// Wraps a node graph editor so it can be added to the Deltos editor.
    ///
    /// # Parameters
    /// * editor: Describes the editor's name and file types.  Opened files get their own graph
    ///   from `NodeGraphEditor::open`.
    /// * font: Font used for tile titles and input/output names.
    pub fn new(editor: Editor, font: Arc<SDFFont>) -> Self {
        Self { editor, font }
    }
}

impl <Editor: NodeGraphEditor> FileEditor for NodeGraphFileEditor<Editor> {
    fn name(&self) -> String { self.editor.name() }
    fn file_types(&self) -> Vec<String> { self.editor.file_types() }

    fn build_editor(
        &mut self,
        theme: &Theme,
        source: Arc<dyn Source>,
        path: PathBuf,
        read_only: bool
    ) -> BoxFuture<'static, UiBundle> {
        let (theme, font) = (*theme, self.font.clone());
        Box::pin(async move {
            // read the file into its own graph
            let opened = source
                .read_file_text_versioned(path.clone())
                .await
                .and_then(|(text, version)| Ok((Editor::open(&text)?, version)));
            let (editor, version) = match opened {
                Ok(opened) => opened,
                Err(err) => return UiBundle::new(Node::new().padding(Rect::single(px(12.0))))
                    .text(Text::new(font, format!("Couldn't open {}: {err:#}", path.display()), theme.red, 14.0))
            };
            let file = OpenFile { source: source.id(), path, dirty: false, read_only, version };
            graph_bundle(&theme, &font, editor, file)
        })
    }
}

/// Builds the view of an opened file's graph: its wires, tiles, and new tile menu over a pannable
/// background that holds the graph and the file it was opened from.
fn graph_bundle<Editor: NodeGraphEditor>(theme: &Theme, font: &Arc<SDFFont>, editor: Editor, file: OpenFile) -> UiBundle {
    // build the tiles, wires and new tile menu before the graph is shared
    let tiles: Vec<UiBundle> = editor
        .tiles()
        .iter()
        .map(|tile| tile_bundle(theme, font, tile, Vec2::ZERO, file.read_only))
        .collect();
    let wires: Vec<UiBundle> = wire_nodes(&editor, theme)
        .into_iter()
        .map(wire_bundle)
        .collect();
    let new_tile_menu = new_tile_menu(editor.new_tile_options());
    let graph: Arc<dyn SharedGraph> = Arc::new(Mutex::new(editor));

    UiBundle::new(
        Node::row()
            .grow(1.0)
            .width(Val::Percent(1.0))
            .min_width(px(0.0))
            .min_height(px(0.0))
            .justify(JustifyContent::Center)
            .align(AlignItems::Center)
            .gap(px(24.0))
            .background(theme.bg_0)
    )
        .with(GraphRoot { graph, pan: Vec2::ZERO })
        .with(file)
        .interactive()
        .context_menu(new_tile_menu)
        .children(wires)
        .children(tiles)
        .child(preview_bundle())
        .child(ring_bundle())
}

/// Builds one tile: a rounded box with a colored title bar over its inputs and outputs, then a
/// field for each of its properties, which can't be edited when `read_only` is set.
///
/// Nodes clip their children, so the tile is a transparent wrapper half a dot wider than its box
/// on each side.  The box is drawn behind the content, which lets the input and output dots
/// overlap its left and right edges.  The tile is placed at its position moved by `offset`, the
/// graph's pan.
pub(crate) fn tile_bundle<T: NodeGraphTile>(theme: &Theme, font: &Arc<SDFFont>, tile: &T, offset: Vec2, read_only: bool) -> UiBundle {
    let tile_id = tile.tile_id();
    let position = tile.position() + offset;
    let overhang = IO_DOT / 2.0;
    let inner_radius = TILE_RADIUS - TILE_BORDER;

    // outer box, drawn behind the title bar and body
    let outer_box = UiBundle::new(
        Node::new()
            .absolute(Rect::new(px(0.0), px(0.0), px(overhang), px(overhang)))
            .z_index(-1)
            .background(theme.bg_2)
            .border(TILE_BORDER, theme.bg_4)
            .radius(px(TILE_RADIUS))
    );

    // title bar with a centered title, rounded on top to match the outer box
    let mut title_bar = Node::row()
        .justify(JustifyContent::Center)
        .padding(Rect::new(px(6.0), px(6.0), px(12.0), px(12.0)))
        .background(tile.color(theme));
    title_bar
        .style_mut()
        .set_border_radius(RectCorners::new(px(inner_radius), px(inner_radius), px(0.0), px(0.0)));
    let title_bar = UiBundle::new(title_bar)
        .child(UiBundle::new(Node::new()).text(Text::new(font.clone(), tile.title(), theme.bg_0, TITLE_SIZE)));

    // inputs down the left, outputs down the right
    let inputs = tile
        .inputs(theme)
        .into_iter()
        .map(|input| io_row(theme, font, tile_id, &input, false));
    let outputs = tile
        .outputs(theme)
        .into_iter()
        .map(|(output, _)| io_row(theme, font, tile_id, &output, true));

    // body pulled out past the border so the dots are centered on the middle of the box's border
    let pull = -(overhang + TILE_BORDER / 2.0);
    let body = UiBundle::new(
        Node::row()
            .justify(JustifyContent::SpaceBetween)
            .gap(px(24.0))
            .padding(Rect::new(px(10.0), px(10.0), px(0.0), px(0.0)))
            .margin(Rect::new(px(0.0), px(0.0), px(pull), px(pull)))
    )
    .child(UiBundle::new(Node::column().gap(px(6.0)).align(AlignItems::Start)).children(inputs))
    .child(UiBundle::new(Node::column().gap(px(6.0)).align(AlignItems::End)).children(outputs));

    // property fields under the inputs and outputs
    let properties = tile
        .properties()
        .into_iter()
        .map(|property| property_row(theme, font, tile_id, &property, read_only))
        .collect::<Vec<_>>();
    let properties = UiBundle::new(
        Node::column()
            .gap(px(6.0))
            .padding(Rect::new(px(0.0), px(10.0), px(12.0), px(12.0)))
    )
    .children(properties);

    let tile = UiBundle::new(
        Node::column()
            .absolute(Rect::new_top_left(px(position.y), px(position.x)))
            .padding(Rect::new(
                px(TILE_BORDER),
                px(TILE_BORDER),
                px(overhang + TILE_BORDER),
                px(overhang + TILE_BORDER)
            ))
    )
    .with(TileNode { tile_id })
    .interactive();

    // tiles of a read only graph can't be deleted, so they get no menu
    let tile = tile
        .child(outer_box)
        .child(title_bar)
        .child(body)
        .child(properties);
    if read_only { tile } else { tile.context_menu(tile_menu()) }
}

/// Builds a property's row: its name then a text field holding its value.
fn property_row(theme: &Theme, font: &Arc<SDFFont>, tile_id: Uuid, property: &TileProperty, read_only: bool) -> UiBundle {
    let name = UiBundle::new(Node::new())
        .text(Text::new(font.clone(), property.name(), theme.bg_8, IO_SIZE));
    let field = UiBundle::new(
        Node::new()
            .width(px(PROPERTY_FIELD_WIDTH))
            .padding(Rect::new(px(2.0), px(2.0), px(6.0), px(6.0)))
            .background(theme.bg_0)
            .border(1.0, theme.bg_4)
            .radius(px(4.0))
    )
    .text(Text::new(font.clone(), "", theme.bg_8, IO_SIZE))
    .with(property_input(property.value(), read_only))
    .with(PropertyField { tile_id, name: property.name().clone() })
    .interactive();

    UiBundle::new(
        Node::row()
            .justify(JustifyContent::SpaceBetween)
            .align(AlignItems::Center)
            .gap(px(12.0))
    )
    .child(name)
    .child(field)
}

/// The text input of a property field showing `value`.
pub(crate) fn property_input(value: &str, read_only: bool) -> TextInput {
    let mut input = TextInput::new(value);
    input.disabled = read_only;
    input
}

/// Builds an input row (dot then name) or, when `output` is set, an output row (name then dot).
/// The dot can be hovered and clicked to make wires.
fn io_row(theme: &Theme, font: &Arc<SDFFont>, tile_id: Uuid, io: &IOButton, output: bool) -> UiBundle {
    let dot = UiBundle::new(
        Node::new()
            .size(px(IO_DOT), px(IO_DOT))
            .radius(px(IO_DOT / 2.0))
            .background(io.color())
    )
    .with(IODot { tile_id, button_id: io.id(), name: io.name().clone(), output, color: io.color(), rules: io.wire_rules() })
    .interactive();
    let name = UiBundle::new(Node::new())
        .text(Text::new(font.clone(), io.name(), theme.bg_8, IO_SIZE));

    let row = UiBundle::new(Node::row().align(AlignItems::Center).gap(px(6.0)));
    if output { row.child(name).child(dot) } else { row.child(dot).child(name) }
}

/// Every `Wire` out of every tile's outputs in `editor`, each in its output's color.  A wire
/// whose target isn't in the graph stays hidden.
pub(crate) fn wire_nodes<Editor: NodeGraphEditor>(editor: &Editor, theme: &Theme) -> Vec<WireNode> {
    let mut nodes = Vec::new();
    for tile in editor.tiles() {
        for (output, wires) in tile.outputs(theme) {
            nodes.extend(wires.iter().map(|wire| WireNode {
                output_tile: tile.tile_id(),
                output_button: output.id(),
                input_tile: wire.target_tile(),
                input_button: wire.target_input(),
                color: output.color(),
                drawn: None,
                hovered: false
            }));
        }
    }
    nodes
}

/// Builds the menu shown when a tile is right clicked, which only has a "Delete" item for now.
fn tile_menu() -> UiBundle {
    let delete = themed::menu_item("Delete").with(DeleteTileItem);
    themed::menu(TILE_MENU_WIDTH, [delete])
}

/// Builds the menu of new tiles shown when a graph's background is right clicked, one item per
/// option from `NodeGraphEditor::new_tile_options`.
fn new_tile_menu(options: Vec<String>) -> UiBundle {
    let items = options
        .into_iter()
        .map(|name| themed::menu_item(name.clone()).with(NewTileItem { name }));
    themed::menu(NEW_TILE_MENU_WIDTH, items)
}

/// Builds a hidden wire, drawn behind the tiles once `draw_wires` finds both of its dots.
pub(crate) fn wire_bundle(wire: WireNode) -> UiBundle {
    UiBundle::new(
        Node::new()
            .absolute(Rect::new_top_left(px(0.0), px(0.0)))
            .z_index(-1)
            .display(Display::None)
    )
    .with(wire)
}

/// Builds the hidden preview of a wire being made, drawn over the tiles.
fn preview_bundle() -> UiBundle {
    UiBundle::new(
        Node::new()
            .absolute(Rect::new_top_left(px(0.0), px(0.0)))
            .z_index(1)
            .display(Display::None)
    )
    .with(WirePreview)
}

/// Builds the hidden ring shown around a hovered dot: a transparent circle with a border, which
/// `hover_rings` colors and moves over the dot.
fn ring_bundle() -> UiBundle {
    UiBundle::new(
        Node::new()
            .absolute(Rect::new_top_left(px(0.0), px(0.0)))
            .size(px(RING_SIZE), px(RING_SIZE))
            .radius(px(RING_SIZE / 2.0))
            .background(Vec4::ZERO)
            .border(RING_WIDTH, Vec4::ZERO)
            .z_index(1)
            .display(Display::None)
    )
    .with(HoverRing)
}

/// Draws a wire `thickness` logical pixels wide into `node` from `start` to `end`, in logical
/// pixels from the graph's top left.
///
/// The wire is a cubic curve that leaves the output and enters the input horizontally.  Nodes
/// can only fill shapes, so the curve is sampled and offset to both sides into a thin closed
/// outline, and `node` is moved and sized to just cover it.
pub(crate) fn draw_wire(node: &mut Node, start: Vec2, end: Vec2, color: Vec4, thickness: f32) {
    let points = wire_points(start, end);

    // offset every sample half the wire's width to each side
    let half = thickness / 2.0;
    let normals: Vec<Vec2> = (0..points.len())
        .map(|i| {
            let direction = points[(i + 1).min(WIRE_SAMPLES)] - points[i.saturating_sub(1)];
            direction
                .normalize_or_zero()
                .perp() * half
        })
        .collect();
    let left: Vec<Vec2> = points
        .iter()
        .zip(&normals)
        .map(|(point, normal)| *point + *normal)
        .collect();
    let right: Vec<Vec2> = points
        .iter()
        .zip(&normals)
        .rev()
        .map(|(point, normal)| *point - *normal)
        .collect();

    // cover the outline with the node, a pixel bigger on each side so its edges stay smooth
    let min = left
        .iter()
        .chain(&right)
        .fold(Vec2::INFINITY, |min, point| min.min(*point)) - Vec2::ONE;
    let max = left
        .iter()
        .chain(&right)
        .fold(Vec2::NEG_INFINITY, |max, point| max.max(*point)) + Vec2::ONE;
    let size = (max - min).max(Vec2::ONE);
    let to_unit = |point: &Vec2| (*point - min) / size - Vec2::splat(0.5);

    // trace down one side and back up the other, closed across the ends
    let left: Vec<Vec2> = left.iter().map(to_unit).collect();
    let right: Vec<Vec2> = right.iter().map(to_unit).collect();
    let builder = smooth_through(Outline::builder().move_to(left[0]), &left);
    let builder = smooth_through(builder.line_to(right[0]), &right);
    let outline = builder.build();

    set_top_left(node, min);
    node.style_mut()
        .set_width(px(size.x))
        .set_height(px(size.y))
        .set_background(Background::Shape(outline, color));
}

/// How far `point` is from the middle of the wire from `start` to `end`, all in logical pixels
/// from the graph's top left.  Used to find the wire under the pointer.
pub(crate) fn wire_distance(start: Vec2, end: Vec2, point: Vec2) -> f32 {
    wire_points(start, end)
        .windows(2)
        .map(|segment| {
            // distance to the closest point on this straight piece of the wire
            let along = segment[1] - segment[0];
            let t = ((point - segment[0]).dot(along) / along.length_squared().max(f32::EPSILON)).clamp(0.0, 1.0);
            point.distance(segment[0] + along * t)
        })
        .fold(f32::INFINITY, f32::min)
}

/// Samples along the wire from `start` to `end`: a cubic curve that leaves the output and enters
/// the input horizontally, bending more the further apart they are.
fn wire_points(start: Vec2, end: Vec2) -> Vec<Vec2> {
    let bend = ((end.x - start.x).abs() / 2.0).max(WIRE_MIN_BEND);
    let (control_a, control_b) = (start + Vec2::new(bend, 0.0), end - Vec2::new(bend, 0.0));
    (0..=WIRE_SAMPLES)
        .map(|i| cubic(start, control_a, control_b, end, i as f32 / WIRE_SAMPLES as f32))
        .collect()
}

/// Continues `builder`, which is at `points[0]`, smoothly through the rest of `points`: each
/// point between the ends bends a curve between the midpoints on either side of it.
fn smooth_through(builder: OutlineBuilder, points: &[Vec2]) -> OutlineBuilder {
    let Some(last) = points.last() else { return builder };
    points
        .windows(2)
        .skip(1)
        .fold(builder, |builder, pair| builder.quad_to(pair[0], (pair[0] + pair[1]) / 2.0))
        .line_to(*last)
}

/// The point `t` of the way along the cubic curve from `a` to `d` pulled toward `b` and `c`.
fn cubic(a: Vec2, b: Vec2, c: Vec2, d: Vec2, t: f32) -> Vec2 {
    let u = 1.0 - t;
    a * (u * u * u) + b * (3.0 * u * u * t) + c * (3.0 * u * t * t) + d * (t * t * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cubic_runs_from_start_to_end() {
        let (a, b, c, d) = (Vec2::ZERO, Vec2::new(10.0, 0.0), Vec2::new(20.0, 30.0), Vec2::new(30.0, 30.0));
        assert_eq!(cubic(a, b, c, d, 0.0), a);
        assert_eq!(cubic(a, b, c, d, 1.0), d);
    }

    #[test]
    fn wire_distance_is_zero_on_the_wire() {
        let (start, end) = (Vec2::new(0.0, 0.0), Vec2::new(200.0, 100.0));
        assert!(wire_distance(start, end, start) < 0.01);
        assert!(wire_distance(start, end, end) < 0.01);
        assert!(wire_distance(start, end, (start + end) / 2.0) < 0.01, "the curve is symmetric about its middle");
        assert!(wire_distance(start, end, Vec2::new(0.0, 100.0)) > 20.0);
    }

    #[test]
    fn wire_node_covers_both_ends() {
        let (start, end) = (Vec2::new(100.0, 50.0), Vec2::new(300.0, 200.0));
        let mut node = Node::new();
        draw_wire(&mut node, start, end, Vec4::ONE, WIRE_THICKNESS);

        // the node's box holds both ends: flat caps along x, half the wire's width above and below
        let PositionType::Absolute(edges) = node.style().position_type() else { panic!("wire isn't absolute") };
        let (Val::Px(left), Val::Px(top)) = (*edges.left(), *edges.top()) else { panic!("wire isn't placed in pixels") };
        let (Val::Px(width), Val::Px(height)) = (*node.style().width(), *node.style().height()) else { panic!("wire isn't sized in pixels") };
        let (min, max) = (Vec2::new(left, top), Vec2::new(left + width, top + height));
        for point in [start, end] {
            let room = Vec2::new(0.0, WIRE_THICKNESS / 2.0);
            assert!(point.cmpge(min + room).all() && point.cmple(max - room).all(), "{point} outside {min}..{max}");
        }
        assert!(matches!(node.style().background(), Background::Shape(_, color) if *color == Vec4::ONE));
    }
}
