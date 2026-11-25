use std::{fmt::Debug, path::PathBuf, sync::Arc};

use deltos::{editors::FileEditor, sources::{BoxFuture, Source}, theme::Theme};
use gearbox::ui::*;

use crate::{IOButton, NodeGraphTile};

/// Font size of a tile's title.
const TITLE_SIZE: f32 = 18.0;

/// Font size of input and output names.
const IO_SIZE: f32 = 14.0;

/// Corner radius of a tile's outer box.
const TILE_RADIUS: f32 = 8.0;

/// Border width of a tile's outer box.
const TILE_BORDER: f32 = 1.5;

/// Diameter of an input or output dot.
const IO_DOT: f32 = 10.0;

/// Node Graph Editor
///
/// Implemented by anything that describes a node graph for a set of file types.  Wrapped in a
/// `NodeGraphFileEditor` so the Deltos editor can open files with it.
pub trait NodeGraphEditor: Debug + Send {
    /// The display name of this editor.
    fn name(&self) -> String;

    /// A list of file types this editor works on.
    fn file_types(&self) -> Vec<String>;

    /// The tiles of the graph to draw.
    fn graph(&self) -> Vec<Box<dyn NodeGraphTile>>;
}

/// Adapts a `NodeGraphEditor` into a Deltos `FileEditor` that draws its tiles.
///
/// Fields:
/// * editor: The node graph editor whose tiles are drawn.
/// * font: Font used for tile titles and input/output names.
#[derive(Debug)]
pub struct NodeGraphFileEditor {
    editor: Box<dyn NodeGraphEditor>,
    font: Arc<SDFFont>
}

impl NodeGraphFileEditor {
    /// Wraps a node graph editor so it can be added to the Deltos editor.
    ///
    /// # Parameters
    /// * editor: The node graph editor whose tiles are drawn.
    /// * font: Font used for tile titles and input/output names.
    pub fn new(editor: Box<dyn NodeGraphEditor>, font: Arc<SDFFont>) -> Self {
        Self { editor, font }
    }
}

impl FileEditor for NodeGraphFileEditor {
    fn name(&self) -> String { self.editor.name() }
    fn file_types(&self) -> Vec<String> { self.editor.file_types() }

    fn build_editor(
        &mut self,
        theme: &Theme,
        _source: Arc<dyn Source>,
        _path: PathBuf,
        _read_only: bool
    ) -> BoxFuture<'static, UiBundle> {
        let (theme, font, tiles) = (*theme, self.font.clone(), self.editor.graph());
        Box::pin(async move {
            let tiles = tiles
                .iter()
                .map(|tile| tile_bundle(&theme, &font, tile.as_ref()));

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
            .children(tiles)
        })
    }
}

/// Builds one tile: a rounded box with a colored title bar over its inputs and outputs.
///
/// Nodes clip their children, so the tile is a transparent wrapper half a dot wider than its box
/// on each side.  The box is drawn behind the content, which lets the input and output dots
/// overlap its left and right edges.
fn tile_bundle(theme: &Theme, font: &Arc<SDFFont>, tile: &dyn NodeGraphTile) -> UiBundle {
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
        .map(|input| io_row(theme, font, &input, false));
    let outputs = tile
        .outputs(theme)
        .into_iter()
        .map(|output| io_row(theme, font, &output, true));

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

    UiBundle::new(
        Node::column()
            .padding(Rect::new(
                px(TILE_BORDER),
                px(TILE_BORDER),
                px(overhang + TILE_BORDER),
                px(overhang + TILE_BORDER)
            ))
    )
    .child(outer_box)
    .child(title_bar)
    .child(body)
}

/// Builds an input row (dot then name) or, when `output` is set, an output row (name then dot).
fn io_row(theme: &Theme, font: &Arc<SDFFont>, io: &IOButton, output: bool) -> UiBundle {
    let dot = UiBundle::new(
        Node::new()
            .size(px(IO_DOT), px(IO_DOT))
            .radius(px(IO_DOT / 2.0))
            .background(io.color())
    );
    let name = UiBundle::new(Node::new())
        .text(Text::new(font.clone(), io.name(), theme.bg_8, IO_SIZE));

    let row = UiBundle::new(Node::row().align(AlignItems::Center).gap(px(6.0)));
    if output { row.child(name).child(dot) } else { row.child(dot).child(name) }
}
