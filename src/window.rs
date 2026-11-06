use std::path::PathBuf;

use anarchy::*;
use gearbox::{glam::{Vec2, Vec4}, ui::*, *};
use winit::window::{CursorIcon, ResizeDirection};

#[cfg(windows)]
use winit::platform::windows::WindowAttributesExtWindows;

use crate::{DeltosEditor, render::{self, *}};

pub const MENU_HOVER: Vec4 = Vec4::new(0.09, 0.09, 0.09, 1.0);
pub const TAB_SCROLL_PER_NOTCH: f32 = 48.0;
pub const GRIP: f32 = 5.0;
pub const FOLDER_CLOSED: &str = "\u{ea83}";
pub const FOLDER_OPEN: &str = "\u{eaf7}";
pub const FILE_ICON: &str = "\u{eb60}";
pub const INDENT: f32 = 14.0;

pub struct WindowPlugin;
impl Plugin for WindowPlugin {
    fn build(self, app: App) -> App {
        app
            .add_resource(WindowCursor(CursorIcon::Default))
            .with_window_attributes(|attribs| {
                let attribs = attribs.with_decorations(false);
                #[cfg(windows)]
                let attribs = attribs.with_corner_preference(winit::platform::windows::CornerPreference::Round);
                attribs
            })
            .on_startup(setup)
            .on_render_update(window_buttons)
            .on_render_update(drag_window)
            .on_render_update(light_window_buttons)
            .on_render_update(scroll_tabs)
            .on_render_update(resize_window)
            .on_render_update(open_folders)
            .on_render_update(click_files)
    }
}

#[derive(Debug, Clone, Copy, Component)]
pub enum WindowButton { Minimize, Maximize, Close }

#[derive(Debug, Clone, Copy, Component)]
pub struct DragRegion;

#[derive(Debug, Clone, Copy, Component)]
pub struct TabStrip;

#[derive(Debug, Clone, Copy, Component)]
pub struct ResizeEdge(pub ResizeDirection);

#[derive(Debug, Resource)]
pub struct WindowCursor(pub CursorIcon);

#[derive(Debug, Clone, Component)]
pub struct FolderItem {
    pub path: PathBuf,
    pub depth: usize,
    pub loaded: bool,
}

#[derive(Debug, Clone, Component)]
pub struct FileItem {
    pub path: PathBuf
}

fn setup(world: World, editor: Res<DeltosEditor>) {
    world.spawn(
        EntityBuilder::new()
            .push(Transform::new(glam::Vec3::new(0.0, 0.0, 6.0), glam::Quat::IDENTITY, glam::Vec3::ONE))
            .push(Camera::default())
    );

    let root = render::render(&editor);

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

/// Lights caption buttons under the pointer, the close button in red.
fn light_window_buttons(mut buttons: Query<(&'static WindowButton, &'static Interaction, &'static mut Node)>) {
    while let Ok(Some((_, (button, interaction, mut node)))) = buttons.next() {
        let background = match (*interaction, *button) {
            (Interaction::None, _) => Background::Empty,
            (_, WindowButton::Close) => Background::Color(Vec4::new(0.77, 0.17, 0.11, 1.0)),
            _ => Background::Color(MENU_HOVER),
        };
        node.style_mut().set_background(background);
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

/// Lists a folder's contents the first time it opens, and swaps its header's icons as it opens
/// and closes. gearbox's `Collapsible` already shows and hides the contents.
fn open_folders(world: World, editor: Res<DeltosEditor>, toggled: Event<ValueChange<bool>>) {
    for ValueChange { entity, value: open } in toggled.read() {
        let Ok(Some(mut folder)) = world.handle(entity).get::<&mut FolderItem>() else { continue };
        let load = open && !folder.loaded;
        folder.loaded |= open;
        let (path, depth) = (folder.path.clone(), folder.depth);

        // release the component before spawning into the world
        drop(folder);

        let children = world.hierarchy().children(entity).map(|children| children.to_vec()).unwrap_or_default();
        let [header, content, ..] = children[..] else { continue };

        if let Ok(Some(mut node)) = world.handle(header).get::<&mut Node>()
            && let Some(text) = node.style_mut().text_mut()
        {
            let (from, to) = if open { (FOLDER_CLOSED, FOLDER_OPEN) } else { (FOLDER_OPEN, FOLDER_CLOSED) };
            text.content = text.content.replacen(from, to, 1);
        }

        if load {
            for item in folder_contents(&editor, &path, depth + 1) {
                world.spawn_with_parent(Some(content), item.into_entity_builder());
            }
        }
    }
}

/// Open a file from the left bar.
fn click_files(world: World, input: Res<UiInput>) {
    let Some(id) = input.hovered else { return };
    if !input.activated(id) { return }
    let Ok(Some(file)) = world.handle(id).get::<&FileItem>() else { return };

    println!("TODO open file {file:?}")
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
