use std::fmt::Debug;

use anarchy::{Component, Query, Res, ResMut, Resource, World};
use gearbox::{App, Graphics, Plugin, ui::*};
use uuid::Uuid;
use winit::window::CursorIcon;

use crate::{DeltosEditor, SIDE_BAR_UUID_NAMESPACE, hover::HoverStyle};

pub mod file_selector;

/// Width and height of a side bar button, in logical pixels.
pub const BUTTON_SIZE: f32 = 35.0;

/// Room around the buttons in a side bar's button column, in logical pixels.
pub const BUTTON_PADDING: f32 = 5.0;

/// Width of the grab area on the inner edge of a side bar panel, in logical pixels.
pub const RESIZE_GRIP: f32 = 5.0;

/// Smallest width a side bar panel can be dragged to, in logical pixels.
pub const MIN_PANEL_WIDTH: f32 = 120.0;

/// Largest fraction of the window's width a side bar panel can be dragged to.
pub const MAX_PANEL_FRACTION: f32 = 0.45;

/// Fraction of the window's width a side bar panel starts at.
pub const DEFAULT_PANEL_FRACTION: f32 = 0.25;

/// Adds the systems that select, hide and resize the side bars.
///
/// Added by every plugin that registers a `SideBarElement`; gearbox only builds it once.
pub struct SideBarPlugin;
impl Plugin for SideBarPlugin {
    fn build(self, app: App) -> App {
        app .add_resource(SideBarCursor::default())
            .on_render_update(click_icons)
            .on_render_update(resize_panels)
    }
}

/// An element that may be added to the left or right
/// auxilary slots of the editor.
pub trait SideBarElement: Debug + Send {
    /// The ID of this side bar element derived from the elements name unless overriden.
    fn id(&self) -> Uuid { Uuid::new_v5(&SIDE_BAR_UUID_NAMESPACE, self.name().as_bytes()) }

    /// The display name of this side bar element.
    fn name(&self) -> String;

    /// The text based icon of this side bar element.
    fn icon(&self) -> String;

    /// The default slot this sid bar element should take.
    fn default_slot(&self) -> SideBarSlot;

    /// Build the `UiBundle` of this element.
    fn build(&self, world: &World, editor: &DeltosEditor) -> UiBundle;
}

/// Which auxilary slot a `SideBarElement` should or is taking.
///
/// Variants:
/// * Left: The side bar left of the editor tiles.
/// * Right: The side bar right of the editor tiles.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SideBarSlot {
    #[default]
    Left,
    Right
}

/// The resizable panel of one side bar, holding the selected element's content.
///
/// Fields:
/// * slot: Which side bar this panel belongs to.
/// * selected: ID of the `SideBarElement` shown in the panel, if any.
/// * open: Whether the panel is shown or hidden.
#[derive(Debug, Clone, Copy, Component)]
pub struct SideBarPanel {
    pub slot: SideBarSlot,
    pub selected: Option<Uuid>,
    pub open: bool
}

/// The node inside a `SideBarPanel` that the selected element is built into.
///
/// Fields:
/// * slot: Which side bar this content belongs to.
#[derive(Debug, Clone, Copy, Component)]
pub struct SideBarContent {
    pub slot: SideBarSlot
}

/// A button in a side bar's button column that selects a `SideBarElement`.
///
/// Fields:
/// * slot: Which side bar this button belongs to.
/// * id: ID of the `SideBarElement` this button selects.
#[derive(Debug, Clone, Copy, Component)]
pub struct SideBarIcon {
    pub slot: SideBarSlot,
    pub id: Uuid
}

/// The grab area on the inner edge of a `SideBarPanel` that resizes it when dragged.
///
/// Fields:
/// * slot: Which side bar this handle resizes.
#[derive(Debug, Clone, Copy, Component)]
pub struct SideBarResizeHandle {
    pub slot: SideBarSlot
}

/// Tracks whether the side bars changed the window's cursor, so it is only reset when they did.
///
/// Fields:
/// * resizing: The cursor is showing resize arrows for a side bar handle.
#[derive(Debug, Default, Resource)]
pub struct SideBarCursor {
    pub resizing: bool
}

/// Render the left and right sidebars into a pair of `UiBundle`s.
///
/// Each side bar is a column of element buttons beside a resizable panel showing the first
/// element of that side.  A side with no elements is hidden entirely.
///
/// # Parameters
/// * world: World passed to each element's `build`.
/// * editor: Editor whose side bar elements and theme are rendered.
pub fn render_left_right(world: &World, editor: &DeltosEditor) -> (UiBundle, UiBundle) {
    (render_side(world, editor, SideBarSlot::Left), render_side(world, editor, SideBarSlot::Right))
}

/// Builds one side bar: its button column and its panel, ordered so the buttons sit on the
/// window's edge.
fn render_side(world: &World, editor: &DeltosEditor, slot: SideBarSlot) -> UiBundle {
    // collect this side's elements in a stable order
    let mut elements: Vec<_> = editor.side_bars
        .values()
        .filter(|a| a.default_slot() == slot)
        .collect();
    elements.sort_by_key(|a| a.name());

    let selected = elements
        .first()
        .map(|a| a.id());
    let content = elements
        .first()
        .map(|a| a.build(world, editor))
        .unwrap_or_else(|| UiBundle::new(Node::new()));
    let icons: Vec<_> = elements
        .iter()
        .map(|a| (a.id(), a.icon()))
        .collect();

    // hide the whole side bar, buttons included, when no element uses this side
    let display = if icons.is_empty() { Display::None } else { Display::Flex };
    let buttons = button_column(editor, slot, icons, selected);
    let panel = panel(editor, slot, selected, content);

    let root = UiBundle::new(Node::row()
        .shrink(0.0)
        .display(display)
        .background(editor.theme.bg_1));
    match slot {
        SideBarSlot::Left => root
            .child(buttons)
            .child(panel),
        SideBarSlot::Right => root
            .child(panel)
            .child(buttons),
    }
}

/// Builds a side bar's panel around its content, with a resize handle on the edge facing the
/// editor tiles.
fn panel(editor: &DeltosEditor, slot: SideBarSlot, selected: Option<Uuid>, content: UiBundle) -> UiBundle {
    let width = Val::PercentWidth(DEFAULT_PANEL_FRACTION);
    let (zero, auto) = (px(0.0), Val::Auto);
    let edges = match slot {
        SideBarSlot::Left => Rect::new(zero, zero, auto, zero),
        SideBarSlot::Right => Rect::new(zero, zero, zero, auto),
    };

    UiBundle::new(Node::new()
        .width(width)
        .min_width(width)
        .max_width(width)
        .height(Val::Percent(1.0))
        .shrink(0.0))
        .with(SideBarPanel { slot, selected, open: true })
        .child(UiBundle::new(Node::new()
            .width(Val::Percent(1.0))
            .height(Val::Percent(1.0)))
            .with(SideBarContent { slot })
            .child(content))
        .child(UiBundle::new(Node::new()
            .absolute(edges)
            .width(px(RESIZE_GRIP))
            .z_index(10))
            .with(SideBarResizeHandle { slot })
            .with(HoverStyle::new(editor.theme.bg_4))
            .interactive())
}

/// Builds the column of buttons that select a side bar's elements.
fn button_column(editor: &DeltosEditor, slot: SideBarSlot, icons: Vec<(Uuid, String)>, selected_id: Option<Uuid>) -> UiBundle {
    // wide enough for a button plus the padding on both sides, so buttons don't spill out
    let width = px(BUTTON_SIZE + BUTTON_PADDING * 2.0);
    let padding = px(BUTTON_PADDING);

    UiBundle::new(Node::column()
        .padding(Rect::new(padding, padding, padding, padding))
        .gap(px(4.0))
        .min_width(width)
        .max_width(width)
        .shrink(0.0)
        .background(editor.theme.bg_0))
        .children(
            icons.into_iter()
                .map(|(id, icon)|
                    UiBundle::new(Node::new()
                        .width(px(BUTTON_SIZE))
                        .height(px(BUTTON_SIZE))
                        .justify(JustifyContent::Center)
                        .align(AlignItems::Center)
                        .radius(px(5.0)))
                        .text(Text::new(editor.font.clone(), icon, icon_color(editor, Some(id) == selected_id), 24.0)
                            .with_align(Align::Center, Align::Center))
                        .with(SideBarIcon { slot, id })
                        .with(HoverStyle::new(editor.theme.bg_4))
                        .interactive()
                )
        )
}

/// The color of a side bar button's icon, highlighted while its element is shown.
fn icon_color(editor: &DeltosEditor, active: bool) -> gearbox::glam::Vec4 {
    if active { editor.theme.blue } else { editor.theme.bg_8 }
}

/// Selects a side bar element when its button is clicked, or hides and shows the panel when the
/// button of the element already shown is clicked.
fn click_icons(world: World, editor: Res<DeltosEditor>, input: Res<UiInput>) {
    let Some(id) = input.hovered else { return };
    if !input.activated(id) { return }
    let Ok(Some(icon)) = world.handle(id).get::<&SideBarIcon>() else { return };
    let icon = *icon;

    // toggle the panel when its shown element is clicked, otherwise switch to the clicked one
    let mut rebuild = false;
    let mut active = None;
    let mut panels = Query::<(&mut SideBarPanel, &mut Node)>::new(&world);
    while let Ok(Some((_, (mut panel, mut node)))) = panels.next() {
        if panel.slot != icon.slot { continue }
        if panel.selected == Some(icon.id) {
            panel.open = !panel.open;
        } else {
            panel.selected = Some(icon.id);
            panel.open = true;
            rebuild = true;
        }

        let display = if panel.open { Display::Flex } else { Display::None };
        if *node.style().display() != display { node.style_mut().set_display(display); }
        active = panel.open.then_some(icon.id);
    }
    drop(panels);

    // highlight only the button of the shown element
    let mut icons = Query::<(&SideBarIcon, &mut Node)>::new(&world);
    while let Ok(Some((_, (other, mut node)))) = icons.next() {
        if other.slot != icon.slot { continue }
        if let Some(text) = node.style_mut().text_mut() {
            text.color = icon_color(&editor, active == Some(other.id));
        }
    }
    drop(icons);

    if rebuild { rebuild_content(&world, &editor, icon.slot, icon.id) }
}

/// Replaces the content of a side bar's panel with a freshly built element.
fn rebuild_content(world: &World, editor: &DeltosEditor, slot: SideBarSlot, element: Uuid) {
    let Some(side_bar) = editor.side_bars.get(&element) else { return };
    let Some((content, _)) = Query::<&SideBarContent>::new(world)
        .iter()
        .find(|(_, content)| content.slot == slot)
        else { return };

    // wipe the previous element before spawning the new one
    let old = world.hierarchy()
        .children(content)
        .map(|children| children.to_vec())
        .unwrap_or_default();
    for child in old {
        world.handle(child).despawn_recursive();
    }

    world.spawn_with_parent(Some(content), side_bar.build(world, editor).into_entity_builder());
}

/// Resizes a side bar's panel while its handle is dragged, and shows resize arrows while a
/// handle is hovered or dragged.
fn resize_panels(world: World, input: Res<UiInput>, graphics: Res<Graphics>, mut cursor: ResMut<SideBarCursor>) {
    // find the handle under or held by the pointer
    let mut hovering = false;
    let mut dragging = None;
    for (id, handle) in Query::<&SideBarResizeHandle>::new(&world).iter() {
        hovering |= input.hovered == Some(id);
        if input.down && input.captured == Some(id) { dragging = Some(handle.slot) }
    }

    // size the dragged panel so its inner edge follows the pointer
    if let Some(slot) = dragging {
        let scale = input.scale.max(f32::EPSILON);
        let max = (input.screen.x / scale * MAX_PANEL_FRACTION).max(MIN_PANEL_WIDTH);
        let mut panels = Query::<(&SideBarPanel, &ComputedNode, &mut Node)>::new(&world);
        while let Ok(Some((_, (panel, computed, mut node)))) = panels.next() {
            if panel.slot != slot { continue }
            let width = match slot {
                SideBarSlot::Left => input.cursor.x - computed.position.x,
                SideBarSlot::Right => computed.position.x + computed.size.x - input.cursor.x,
            };
            let width = px((width / scale).clamp(MIN_PANEL_WIDTH, max));

            // only touch the style when the width actually changes
            if *node.style().width() != width {
                node.style_mut()
                    .set_width(width)
                    .set_min_width(width)
                    .set_max_width(width);
            }
        }
    }

    // only reset the cursor if the side bars set it, so the window's resize edges keep theirs
    let resizing = hovering || dragging.is_some();
    if cursor.resizing != resizing && let Some(window) = graphics.window() {
        window.set_cursor(if resizing { CursorIcon::EwResize } else { CursorIcon::Default });
        cursor.resizing = resizing;
    }
}
