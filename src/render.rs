use std::path::{Path, PathBuf};

use anarchy::World;
use gearbox::{glam::*, ui::*};
use winit::window::ResizeDirection;

use crate::{DeltosEditor, EditorTab, EditorTile, hover::HoverStyle, top_bar, window::*};
use uuid::Uuid;

pub fn render(world: &World, editor: &DeltosEditor) -> UiBundle {
    let tile = editor.tiles.values().next().unwrap();

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
                .child(file_selector(&editor))
                .child(UiBundle::new(Node::new()
                    .width(Val::Percent(1.0))
                    .height(Val::Percent(1.0)))
                    .child(tab_pane(&editor, tile))
                    .child(
                        UiBundle::new(Node::new()
                            .width(Val::Percent(1.0))
                            .height(Val::Percent(1.0))
                            .min_height(px(0.0)))
                            .with(EditorContainer { tile_id: tile.id })
                    ))
        ])
        .children(resize_edges())
}

/// Builds the file list: one list per source, stacked in a single column.
fn file_selector(editor: &DeltosEditor) -> UiBundle {
    // order sources by name so the lists keep a stable order between launches
    let mut sources: Vec<_> = editor.sources.iter().collect();
    sources.sort_by_key(|(_, source)| source.name());

    UiBundle::new(Node::column()
        .min_width(Val::Percent(0.25))
        .max_width(Val::Percent(0.25))
        .height(Val::Percent(1.0))
        .gap(px(SOURCE_GAP))
        .overflow_y(Overflow::Scroll))
        .children(sources
            .into_iter()
            .map(|(id, source)| source_list(editor, *id, &source.name()))
            .collect::<Vec<_>>())
}

/// Builds one source's list: a header with the source's name above its root folder contents.
fn source_list(editor: &DeltosEditor, source: Uuid, name: &str) -> UiBundle {
    UiBundle::new(Node::column().shrink(0.0))
        .child(UiBundle::new(Node::new()
            .shrink(0.0)
            .padding(Rect::new(px(8.0), px(6.0), px(12.0), px(12.0))))
            .text(Text::new(editor.font.clone(), name.to_uppercase(), editor.theme.bg_5, 12.0)
                .with_align(Align::Start, Align::Center)))
        .children(folder_contents(editor, source, Path::new("."), 0))
}

/// Builds the file tree rows for the contents of `path` in the source `source`.
pub(crate) fn folder_contents(editor: &DeltosEditor, source: Uuid, path: &Path, depth: usize) -> Vec<UiBundle> {
    let error_row = |message: String| vec![row(depth).text(Text::new(editor.font.clone(), message, editor.theme.red, 12.0))];

    // list the folder from its source
    let Some(file_source) = editor.sources.get(&source) else { return error_row(format!("Unknown source {source}")) };
    let list = match file_source.list_files(path.to_path_buf()) {
        Ok(list) => list,
        Err(err) => return error_row(format!("{err:#}")),
    };

    let folders = list.folders.iter().map(|name| folder_item(editor, source, name.clone(), depth));
    let files = list.files.iter().map(|file| file_item(editor, source, file, depth));
    folders.chain(files).collect()
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

fn tab_pane(editor: &DeltosEditor, tile: &EditorTile) -> UiBundle {
    UiBundle::new(Node::row()
        .width(Val::Percent(1.0))
        .height(px(40.0))
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
