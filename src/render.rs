use anarchy::World;
use gearbox::{glam::*, ui::*};
use winit::window::ResizeDirection;

use crate::{DeltosEditor, EditorTab, EditorTile, hover::HoverStyle, side_bar, tiles::{SplitAxis, TileLayout}, top_bar, window::*};

pub const DIVIDER_SIZE: f32 = 1.5;
pub const TAB_BAR_HEIGHT: f32 = 40.0;

/// Builds the editor's whole UI: the top bar, the side bars and the editor tiles between them.
///
/// # Parameters
/// * world: World passed to the bars' elements as they build.
/// * editor: Editor whose UI is rendered.
pub fn render(world: &World, editor: &DeltosEditor) -> UiBundle {
    let (left_side_bar, right_side_bar) = side_bar::render_left_right(world, editor);

    UiBundle::new(
            Node::column()
                .anchored(Anchor::Center)
                .width(Val::Percent(1.0))
                .height(Val::Percent(1.0))
                .background(editor.theme.bg_0)
        )
        .children([
            // tab bar and top pane
            top_bar::render(&world, &editor),

            // editor pane and central pane, allowed to shrink below their content so they scroll
            // instead of growing past the window
            UiBundle::new(Node::row()
                .grow(1.0)
                .min_height(px(0.0)))
                .child(left_side_bar)
                .child(UiBundle::new(Node::row()
                    .width(Val::Percent(1.0))
                    .height(Val::Percent(1.0))
                    .min_width(px(0.0))
                    .min_height(px(0.0)))
                    .with(TileArea)
                    .child(tile_layout(editor, &editor.layout)))
                .child(right_side_bar)
        ])
        .child(drop_preview(editor))
        .children(resize_edges())
}

/// Builds the editor tiles as `layout` arranges them, each split sharing its space evenly.
///
/// # Parameters
/// * editor: Editor whose tiles and theme are rendered.
/// * layout: The part of the editor's layout to build.
pub fn tile_layout(editor: &DeltosEditor, layout: &TileLayout) -> UiBundle {
    match layout {
        TileLayout::Tile(id) => match editor.tiles.get(id) {
            Some(tile) => tile_node(editor, tile),
            None => grow_node(Node::new()),
        },
        TileLayout::Split { axis, first, second } => {
            // a thin line between the halves, along the split's cross axis
            let (node, divider) = match axis {
                SplitAxis::Horizontal => (Node::row(), Node::new().width(px(DIVIDER_SIZE)).height(Val::Percent(1.0))),
                SplitAxis::Vertical => (Node::column(), Node::new().width(Val::Percent(1.0)).height(px(DIVIDER_SIZE))),
            };
            grow_node(node)
                .child(tile_layout(editor, first))
                .child(UiBundle::new(divider
                    .shrink(0.0)
                    .background(editor.theme.bg_3)))
                .child(tile_layout(editor, second))
        }
    }
}

/// Wraps a node so it takes an even share of its parent and can shrink below its content.
fn grow_node(node: Node) -> UiBundle {
    UiBundle::new(node
        .grow(1.0)
        .shrink(1.0)
        .basis(px(0.0))
        .min_width(px(0.0))
        .min_height(px(0.0)))
}

/// Builds one tile: its tab bar above the container its shown editor goes in.
fn tile_node(editor: &DeltosEditor, tile: &EditorTile) -> UiBundle {
    grow_node(Node::column())
        .with(TileNode { tile_id: tile.id })
        .child(tab_pane(editor, tile))
        .child(
            UiBundle::new(Node::new()
                .width(Val::Percent(1.0))
                .grow(1.0)
                .basis(px(0.0))
                .min_height(px(0.0)))
                .with(EditorContainer { tile_id: tile.id })
        )
}

/// Builds the hidden highlight shown over where a dragged tab will land.
fn drop_preview(editor: &DeltosEditor) -> UiBundle {
    let mut color = editor.theme.blue;
    color.w = 0.25;
    UiBundle::new(Node::new()
        .fixed(Rect::new_top_left(px(0.0), px(0.0)))
        .width(px(0.0))
        .height(px(0.0))
        .z_index(900)
        .display(Display::None)
        .background(color))
        .with(DropPreview)
}

fn tab_pane(editor: &DeltosEditor, tile: &EditorTile) -> UiBundle {
    UiBundle::new(Node::row()
        .width(Val::Percent(1.0))
        .height(px(TAB_BAR_HEIGHT))
        .background(editor.theme.bg_1)
        .align(AlignItems::Center)
        .shrink(0.0))
        .child(tab_strip(editor, tile))
}

fn tab_strip(editor: &DeltosEditor, tile: &EditorTile) -> UiBundle {
    UiBundle::new(Node::row()
        .height(Val::Percent(1.0))
        .align(AlignItems::Center)
        .shrink(1.0)
        .min_width(px(0.0))
        .overflow_x(Overflow::Scroll)
        .scrollbar(Scrollbar { width: 0.0, gap: 0.0, ..Scrollbar::default() }))
        .with(TabStrip { tile_id: tile.id })
        .children(tile.tabs.iter().map(|tab_name| tab(editor, &tile, tab_name.1)))
}

pub(crate) fn tab(editor: &DeltosEditor, tile: &EditorTile, tab: &EditorTab) -> UiBundle {
    let active = tile.editor_tab == Some(tab.id);

    UiBundle::new(Node::row()
        .align(AlignItems::Center)
        .gap(px(12.0))
        .padding(Rect::new(px(0.0), px(0.0), px(10.0), px(10.0)))
        .height(Val::Percent(1.0))
        .border_sides(Rect::new(px(1.5), px(0.0), px(1.5), px(1.5)), editor.theme.bg_3)
        .background(if tile.editor_tab.is_some() && tile.editor_tab.unwrap() == tab.id { editor.theme.bg_4 } else { editor.theme.bg_2 }))
        .with(TabItem { tile_id: tile.id, tab_id: tab.id })
        .with(HoverStyle::new(editor.theme.bg_4).base(if active { editor.theme.bg_4 } else { editor.theme.bg_2 }))
        .interactive()
        .children([
            UiBundle::new(Node::new())
                .text(Text::new(editor.font.clone(), &tab.name, Vec4::ONE, 14.0)
                    .with_align(Align::Center, Align::Center)),
            UiBundle::new(Node::new()
                .width(px(20.0))
                .height(px(20.0))
                .justify(JustifyContent::Center)
                .align(AlignItems::Center)
                .margin(Rect::single(Val::Px(5.0)))
                .radius(Val::Px(5.0)))
                .text(Text::new(editor.font.clone(), "×", Vec4::ONE, 14.0)
                    .with_align(Align::Center, Align::Center))
                .with(TabCloseButton { tile_id: tile.id, tab_id: tab.id })
                .with(HoverStyle::new(editor.theme.red))
                .interactive(),
        ])
}

fn resize_edges() -> Vec<UiBundle> {
    let auto = Val::Auto;
    let edge = |direction: ResizeDirection, edges: Rect, width: Val, height: Val| {
        UiBundle::new(Node::new().absolute(edges).width(width).height(height).z_index(1000))
            .with(ResizeEdge(direction))
            .interactive()
    };
    let (zero, grip, corner) = (px(0.0), px(GRIP), px(GRIP * 2.0));
    vec![
        // sides, stretched between the left and right or top and bottom edges
        edge(ResizeDirection::North, Rect::new(zero, auto, zero, zero), auto, grip),
        edge(ResizeDirection::South, Rect::new(auto, zero, zero, zero), auto, grip),
        edge(ResizeDirection::West, Rect::new(zero, zero, zero, auto), grip, auto),
        edge(ResizeDirection::East, Rect::new(zero, zero, auto, zero), grip, auto),

        // corners last, so they sit over the sides
        edge(ResizeDirection::NorthWest, Rect::new_top_left(zero, zero), corner, corner),
        edge(ResizeDirection::NorthEast, Rect::new_top_right(zero, zero), corner, corner),
        edge(ResizeDirection::SouthWest, Rect::new_bottom_left(zero, zero), corner, corner),
        edge(ResizeDirection::SouthEast, Rect::new_bottom_right(zero, zero), corner, corner),
    ]
}
