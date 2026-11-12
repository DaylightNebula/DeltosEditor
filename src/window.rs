use std::path::{Path, PathBuf};

use anarchy::*;
use gearbox::{glam::{Vec2, Vec4}, ui::*, *};
use uuid::Uuid;
use winit::window::{CursorIcon, ResizeDirection};

#[cfg(windows)]
use winit::platform::windows::WindowAttributesExtWindows;

use crate::{DeltosEditor, TAB_UUID_NAMESPACE, editors::text::OpenFile, render::{self, TAB_BAR_HEIGHT}, tiles::DropZone};

pub const MENU_HOVER: Vec4 = Vec4::new(0.09, 0.09, 0.09, 1.0);
/// How far the pointer must move with a tab held before it starts dragging, in logical pixels.
pub const TAB_DRAG_THRESHOLD: f32 = 6.0;
pub const TAB_SCROLL_PER_NOTCH: f32 = 48.0;
pub const GRIP: f32 = 5.0;
pub const DIRTY_ICON: &str = "\u{ea71}";
pub const CLOSE_ICON: &str = "\u{ea76}";

pub struct WindowPlugin;
impl Plugin for WindowPlugin {
    fn build(self, app: App) -> App {
        app
            .add_resource(WindowCursor(CursorIcon::Default))
            .add_resource(TabDrag::default())
            .with_window_attributes(|attribs| {
                let attribs = attribs.with_decorations(false);
                #[cfg(windows)]
                let attribs = attribs.with_corner_preference(winit::platform::windows::CornerPreference::Round);
                attribs
            })
            // builds the UI, so it runs after every other startup system has registered its parts
            .on_startup_with(setup, pin(SystemPin::End))
            .on_render_update(window_buttons)
            .on_render_update(drag_window)
            .on_render_update(scroll_tabs)
            .on_render_update(resize_window)
            .on_render_update(click_files)
            .on_render_update(drag_tabs)
            .on_render_update(click_tabs)
            .on_render_update(show_dirty_tabs)
    }
}

#[derive(Debug, Clone, Copy, Component, PartialEq, Eq, PartialOrd, Ord)]
pub enum WindowButton { Minimize, Maximize, Close }

#[derive(Debug, Clone, Copy, Component)]
pub struct DragRegion;

#[derive(Debug, Clone, Copy, Component)]
pub struct TabStrip {
    pub tile_id: Uuid
}

#[derive(Debug, Clone, Copy, Component)]
pub struct TabItem {
    pub tile_id: Uuid,
    pub tab_id: Uuid
}

#[derive(Debug, Clone, Copy, Component)]
pub struct TabCloseButton {
    pub tile_id: Uuid,
    pub tab_id: Uuid
}

/// The node a tile's shown editor is built into.
///
/// Fields:
/// * tile_id: ID of the tile this container belongs to.
#[derive(Debug, Clone, Copy, Component)]
pub struct EditorContainer {
    pub tile_id: Uuid
}

/// Marks the root node of a built editor, so it can be moved between tiles with its tab.
///
/// Fields:
/// * tab_id: ID of the tab the editor was built for.
#[derive(Debug, Clone, Copy, Component)]
pub struct EditorContent {
    pub tab_id: Uuid
}

/// The node holding every editor tile, rebuilt whenever the tile layout changes.
#[derive(Debug, Clone, Copy, Component)]
pub struct TileArea;

/// The node of one editor tile, holding its tab bar and editor container.
///
/// Fields:
/// * tile_id: ID of the tile this node shows.
#[derive(Debug, Clone, Copy, Component)]
pub struct TileNode {
    pub tile_id: Uuid
}

/// The highlight over where a dragged tab will be dropped.
#[derive(Debug, Clone, Copy, Component)]
pub struct DropPreview;

/// A tab held by the pointer, which becomes a drag once the pointer moves far enough.
///
/// Fields:
/// * tile_id: ID of the tile the tab is in.
/// * tab_id: ID of the held tab.
/// * start: Where the pointer pressed the tab, in window pixels.
/// * dragging: Whether the pointer moved far enough to start dragging.
/// * target: The tile and zone the tab will drop in, if the pointer is over one.
#[derive(Debug, Clone, Copy)]
pub struct HeldTab {
    pub tile_id: Uuid,
    pub tab_id: Uuid,
    pub start: Vec2,
    pub dragging: bool,
    pub target: Option<(Uuid, DropZone)>
}

/// The tab being dragged, if any.
///
/// Fields:
/// * 0: The held tab, `None` while no tab is pressed.
#[derive(Debug, Default, Resource)]
pub struct TabDrag(pub Option<HeldTab>);

#[derive(Debug, Clone, Copy, Component)]
pub struct ResizeEdge(pub ResizeDirection);

#[derive(Debug, Resource)]
pub struct WindowCursor(pub CursorIcon);

/// A folder row in the file tree.
///
/// Fields:
/// * source: ID of the `Source` the folder belongs to.
/// * path: Path of the folder inside its source.
/// * depth: How deep the folder is nested in the tree.
/// * loaded: Whether the folder's contents have been listed yet.
#[derive(Debug, Clone, Component)]
pub struct FolderItem {
    pub source: Uuid,
    pub path: PathBuf,
    pub depth: usize,
    pub loaded: bool,
}

/// A file row in the file tree.
///
/// Fields:
/// * source: ID of the `Source` the file belongs to.
/// * path: Path of the file inside its source.
#[derive(Debug, Clone, Component)]
pub struct FileItem {
    pub source: Uuid,
    pub path: PathBuf
}

fn setup(world: World, editor: Res<DeltosEditor>) {
    world.spawn(
        EntityBuilder::new()
            .push(Transform::new(glam::Vec3::new(0.0, 0.0, 6.0), glam::Quat::IDENTITY, glam::Vec3::ONE))
            .push(Camera::default())
    );

    let root = render::render(&world, &editor);

    world.spawn(root.into_entity_builder());
}

/// Minimizes, maximizes or closes the window when a caption button is clicked.
fn window_buttons(input: Res<UiInput>, graphics: Res<Graphics>, mut buttons: Query<&'static WindowButton>) {
    let Some(window) = graphics.window() else { return };
    while let Ok(Some((id, button))) = buttons.next() {
        if !input.activated(id) { continue }
        match *button {
            WindowButton::Minimize => window.set_minimized(true),
            WindowButton::Maximize => window.set_maximized(!window.is_maximized()),
            WindowButton::Close => std::process::exit(0),
        }
    }
}

/// Moves the window when the empty tab bar is pressed, and maximizes it on a double click.
fn drag_window(input: Res<UiInput>, graphics: Res<Graphics>, mut regions: Query<&'static DragRegion>) {
    if !input.pressed { return }
    let Some(window) = graphics.window() else { return };
    while let Ok(Some((id, _))) = regions.next() {
        if input.hovered != Some(id) { continue }
        if input.clicks == 2 {
            window.set_maximized(!window.is_maximized());
        } else if let Err(err) = window.drag_window() {
            log::error!("Failed to drag the window: {err}");
        }
    }
}

// Handle scrolling tab strip
fn scroll_tabs(input: Res<UiInput>, mut strips: Query<(&'static TabStrip, &'static ComputedNode, &'static mut Node)>) {
    if input.wheel == 0.0 || input.modifiers.shift_key() || input.over_windows { return }
    let scale = input.scale.max(f32::EPSILON);
    while let Ok(Some((_, (_, computed, mut node)))) = strips.next() {
        let Some(scroll) = computed.scroll else { continue };
        if !computed.contains(input.cursor) { continue }
        let current = *node.style().scroll();
        let x = (current.x - input.wheel * TAB_SCROLL_PER_NOTCH).clamp(0.0, scroll.max.x / scale);
        if x != current.x { node.style_mut().set_scroll(Vec2::new(x, current.y)); }
    }
}

/// Open a file from the left bar.
fn click_files(world: World, mut editor: ResMut<DeltosEditor>, input: Res<UiInput>) {
    let Some(id) = input.hovered else { return };
    if !input.activated(id) { return }
    let Ok(Some(file)) = world.handle(id).get::<&FileItem>() else { return };
    let (source, path) = (file.source, file.path.clone());

    // release the component before the editor spawns into the world
    drop(file);
    editor.open_tab(&world, source, &path);
}

/// Open a tab on click.
fn click_tabs(world: World, mut editor: ResMut<DeltosEditor>, input: Res<UiInput>) {
    let Some(id) = input.hovered else { return };
    if !input.activated(id) { return }

    // copy the uuids out so the component borrows end before the world is mutated
    let handle = world.handle(id);
    let close = handle.get::<&TabCloseButton>().ok().flatten().map(|c| (c.tile_id, c.tab_id));
    let focus = handle.get::<&TabItem>().ok().flatten().map(|t| (t.tile_id, t.tab_id));

    if let Some(tab) = close { editor.close_tab(&world, tab.0, tab.1) }
    else if let Some(tab) = focus { editor.focus_tab(&world, tab.0, tab.1) }
}

/// Drags tabs between tiles: holding a tab and moving the pointer highlights where it will land,
/// and letting go drops it into that tile, or splits the tile when over one of its edges.
fn drag_tabs(world: World, mut editor: ResMut<DeltosEditor>, input: Res<UiInput>, mut drag: ResMut<TabDrag>) {
    // pick up a tab when it is pressed
    if input.pressed && let Some(id) = input.hovered {
        let tab = world.handle(id)
            .get::<&TabItem>()
            .ok()
            .flatten()
            .map(|tab| (tab.tile_id, tab.tab_id));
        drag.0 = tab.map(|(tile_id, tab_id)| HeldTab { tile_id, tab_id, start: input.cursor, dragging: false, target: None });
    }
    let Some(mut held) = drag.0 else { return };

    // drop the tab, or let it go if it never moved far enough to count as a drag
    if !input.down {
        drag.0 = None;
        set_drop_preview(&world, &input, None);
        if held.dragging && let Some((tile_id, zone)) = held.target {
            editor.move_tab(&world, held.tile_id, held.tab_id, tile_id, zone);
        }
        return;
    }

    // start dragging once the pointer leaves the threshold around where it pressed
    let scale = input.scale.max(f32::EPSILON);
    if !held.dragging && input.cursor.distance(held.start) < TAB_DRAG_THRESHOLD * scale { return }
    held.dragging = true;

    // find the tile under the pointer and where on it the tab would land
    let bar = TAB_BAR_HEIGHT * scale;
    let hit = Query::<(&TileNode, &ComputedNode)>::new(&world)
        .iter()
        .find(|(_, (_, computed))| computed.contains(input.cursor))
        .map(|(_, (tile, computed))| {
            let content_position = computed.position + Vec2::new(0.0, bar);
            let content_size = (computed.size - Vec2::new(0.0, bar)).max(Vec2::ZERO);

            // over the tab bar always joins the tile, anywhere else picks by the pointer's spot
            let zone = if input.cursor.y < content_position.y { DropZone::Center }
                else { DropZone::at(content_position, content_size, input.cursor) };
            (tile.tile_id, zone, zone.preview(content_position, content_size))
        });

    held.target = hit.map(|(tile_id, zone, _)| (tile_id, zone));
    drag.0 = Some(held);
    set_drop_preview(&world, &input, hit.map(|(_, _, preview)| preview));
}

/// Shows the drop highlight over `area`, a position and size in window pixels, or hides it.
fn set_drop_preview(world: &World, input: &UiInput, area: Option<(Vec2, Vec2)>) {
    let scale = input.scale.max(f32::EPSILON);
    let mut previews = Query::<(&DropPreview, &mut Node)>::new(world);
    while let Ok(Some((_, (_, mut node)))) = previews.next() {
        let Some((position, size)) = area else {
            if *node.style().display() != Display::None { node.style_mut().set_display(Display::None); }
            continue;
        };

        // styles are in logical pixels, so convert from the window's pixels
        let (position, size) = (position / scale, size / scale);
        let edges = Rect::new_top_left(px(position.y), px(position.x));
        let unchanged = *node.style().display() == Display::Flex
            && matches!(node.style().position_type(), PositionType::Fixed(now) if *now == edges)
            && *node.style().width() == px(size.x)
            && *node.style().height() == px(size.y);
        if unchanged { continue }

        node.style_mut()
            .set_display(Display::Flex)
            .set_position_type(PositionType::Fixed(edges))
            .set_width(px(size.x))
            .set_height(px(size.y));
    }
}

/// Shows a dot in place of a tab's × while its file has unsaved changes, and the × again
/// while the pointer is over it so the tab can still be closed.
fn show_dirty_tabs(world: World) {
    let dirty: Vec<Uuid> = Query::<&OpenFile>::new(&world).iter()
        .filter(|(_, file)| file.dirty)
        .map(|(_, file)| tab_id_for(&file.path))
        .collect();

    let mut buttons = Query::<(&TabCloseButton, &Interaction, &mut Node)>::new(&world);
    while let Ok(Some((_, (button, interaction, mut node)))) = buttons.next() {
        let glyph = if dirty.contains(&button.tab_id) && *interaction == Interaction::None { DIRTY_ICON } else { CLOSE_ICON };

        // only touch the style when the glyph actually changes
        if node.style().text().as_ref().is_some_and(|text| text.content != glyph)
            && let Some(text) = node.style_mut().text_mut()
        {
            text.content = glyph.to_string();
        }
    }
}

/// Starts resizing the window when an edge is pressed, shows the matching arrows over it, and
/// hides the edges while the window is maximized.
fn resize_window(input: Res<UiInput>, graphics: Res<Graphics>, mut cursor: ResMut<WindowCursor>,
                 mut edges: Query<(&'static ResizeEdge, &'static mut Node)>) {
    let Some(window) = graphics.window() else { return };
    let maximized = window.is_maximized();
    let mut wanted = CursorIcon::Default;
    while let Ok(Some((id, (edge, mut node)))) = edges.next() {
        // a maximized window has no edges to drag, so they stop covering the UI
        let display = if maximized { Display::None } else { Display::Flex };
        if *node.style().display() != display { node.style_mut().set_display(display); }
        if maximized || input.hovered != Some(id) { continue }

        wanted = resize_cursor(edge.0);
        if input.pressed && let Err(err) = window.drag_resize_window(edge.0) {
            log::error!("Failed to resize the window: {err}");
        }
    }
    if cursor.0 != wanted {
        window.set_cursor(wanted);
        cursor.0 = wanted;
    }
}

fn resize_cursor(direction: ResizeDirection) -> CursorIcon {
    use ResizeDirection::*;
    match direction {
        North | South => CursorIcon::NsResize,
        East | West => CursorIcon::EwResize,
        NorthWest | SouthEast => CursorIcon::NwseResize,
        NorthEast | SouthWest => CursorIcon::NeswResize,
    }
}

fn tab_id_for(path: &Path) -> Uuid {
    Uuid::new_v5(&TAB_UUID_NAMESPACE, path.as_os_str().as_encoded_bytes())
}
