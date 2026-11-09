use std::path::{Path, PathBuf};

use gearbox::{glam::*, ui::*};
use winit::window::ResizeDirection;

use crate::{DeltosEditor, EditorTab, EditorTile, colors, sources::{FileSource, local::LocalFileSource}, window::*};

pub fn render(editor: &DeltosEditor) -> UiBundle {
    let tile = editor.tiles.values().next().unwrap();

    UiBundle::new(
            Node::column()
                .anchored(Anchor::Center)
                .width(Val::Percent(1.0))
                .height(Val::Percent(1.0))
                .background(colors::bg0)
        )
        .children([
            // tab bar and top pane
            top_pane(&editor),

            // editor pane and central pane
            UiBundle::new(Node::row()
                .grow(1.0))
                .child(file_selector(&editor))
                .child(UiBundle::new(Node::new()
                    .width(Val::Percent(1.0))
                    .height(Val::Percent(1.0)))
                    .child(tab_pane(&editor, tile))
                    .child(
                        UiBundle::new(Node::new()
                            .width(Val::Percent(1.0))
                            .height(Val::Percent(1.0)))
                            .with(EditorContainer { tile_id: tile.id })
                    ))
        ])
        .children(resize_edges())
}

fn top_pane(editor: &DeltosEditor) -> UiBundle {
    UiBundle::new(Node::row()
        .width(Val::Percent(1.0))
        .height(px(40.0))
        .background(colors::bg1)
        .align(AlignItems::Center)
        .shrink(0.0))
        .child(
            UiBundle::new(Node::row()
                .align(AlignItems::Center)
                .height(Val::Percent(1.0))
                .padding(Rect::new(px(10.0), px(10.0), px(14.0), px(14.0))))
                .interactive()
                .children([
                    UiBundle::new(Node::new())
                        .text(Text::new(editor.font.clone(), "".to_string(), Vec4::ONE, 18.0)
                            .with_align(Align::Center, Align::Center)),
                ])
        )
        .child(
            UiBundle::new(Node::new()
                .grow(1.0)
                .min_width(px(0.0))
                .height(Val::Percent(1.0)))
                .with(DragRegion)
                .interactive()
        )
        .children([
            caption_button(editor, "–", WindowButton::Minimize),
            caption_button(editor, "󱓼", WindowButton::Maximize),
            caption_button(editor, "×", WindowButton::Close),
        ])
}

fn file_selector(editor: &DeltosEditor) -> UiBundle {
    UiBundle::new(Node::column()
        .min_width(Val::Percent(0.25))
        .max_width(Val::Percent(0.25))
        .height(Val::Percent(1.0))
        .overflow_y(Overflow::Scroll))
        .children(folder_contents(editor, Path::new("."), 0))
}

pub(crate) fn folder_contents(editor: &DeltosEditor, path: &Path, depth: usize) -> Vec<UiBundle> {
    let list = match LocalFileSource.list_files(path.to_path_buf()) {
        Ok(list) => list,
        Err(err) => return vec![row(depth).text(Text::new(editor.font.clone(), format!("{err:#}"), colors::red, 12.0))],
    };

    let folders = list.folders.iter().map(|name| folder_item(editor, name.clone(), depth));
    let files = list.files.iter().map(|file| file_item(editor, file, depth));
    folders.chain(files).collect()
}

fn folder_item(editor: &DeltosEditor, path: PathBuf, depth: usize) -> UiBundle {
    let name = path.file_name()
        .map(|a| a.to_string_lossy().to_string())
        .unwrap_or_else(|| String::new());

    UiBundle::new(Node::column().shrink(0.0))
        .with(Collapsible { open: false })
        .with(FolderItem { path, depth, loaded: false })
        .child(row(depth)
            .text(Text::new(editor.font.clone(), format!("{FOLDER_CLOSED} {name}"), colors::fg0, 14.0)
                .with_align(Align::Start, Align::Center))
            .interactive())
        .child(UiBundle::new(Node::column().shrink(0.0)))
}

fn file_item(editor: &DeltosEditor, file: &PathBuf, depth: usize) -> UiBundle {
    let name = file.file_name()
        .map(|a| a.to_string_lossy().to_string())
        .unwrap_or_else(|| String::new());

    row(depth)
        .text(Text::new(editor.font.clone(), format!("{FILE_ICON} {name}"), colors::fg0, 14.0)
            .with_align(Align::Start, Align::Center))
        .interactive()
        .with(FileItem { path: file.clone() })
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
        .background(colors::bg1)
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
    UiBundle::new(Node::row()
        .align(AlignItems::Center)
        .gap(px(12.0))
        .padding(Rect::new(px(0.0), px(0.0), px(10.0), px(10.0)))
        .height(Val::Percent(1.0))
        .border_sides(Rect::new(px(1.5), px(0.0), px(1.5), px(1.5)), colors::bg3)
        .background(if tile.editor_tab.is_some() && tile.editor_tab.unwrap() == tab.id { colors::bg4 } else { colors::bg2 }))
        .with(TabItem { tile_id: tile.id, tab_id: tab.id })
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
                .interactive(),
        ])
}

fn caption_button(editor: &DeltosEditor, label: &str, button: WindowButton) -> UiBundle{
    UiBundle::new(Node::row()
        .width(px(46.0))
        .height(Val::Percent(1.0))
        .align(AlignItems::Center)
        .justify(JustifyContent::Center)
        .shrink(0.0))
        .with(button)
        .interactive()
        .text(Text::new(editor.font.clone(), label, Vec4::ONE, 16.0)
            .with_align(Align::Center, Align::Center))
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
