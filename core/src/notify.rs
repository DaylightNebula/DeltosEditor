use anarchy::{Component, Query, Res, ResMut, World};
use gearbox::{App, Plugin, RenderDeltaTime, glam::Vec4, ui::*};

use crate::{DeltosEditor, hover::HoverStyle};

/// How long a toast stays on screen, in seconds.
pub const TOAST_SECONDS: f32 = 6.0;

/// Most toasts shown at once; older ones are dropped first.
pub const MAX_TOASTS: usize = 4;

/// Width of a toast, in logical pixels.
pub const TOAST_WIDTH: f32 = 340.0;

/// Shows the messages passed to `DeltosEditor::notify` as toasts in the editor's bottom right
/// corner.  Added by `DeltosPlugin`.
pub struct NotifyPlugin;
impl Plugin for NotifyPlugin {
    fn build(self, app: App) -> App {
        app .on_render_update(show_toasts)
            .on_render_update(expire_toasts)
    }
}

/// How important a notification is, which picks its color and log level.
///
/// Variants:
/// * Info: Something finished, like a save.
/// * Warning: Something needs the user's attention but nothing was lost.
/// * Error: Something failed, like a source being unreachable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NotifyLevel {
    Info,
    Warning,
    Error
}

/// A message waiting to be shown as a toast, see `DeltosEditor::notify`.
///
/// Fields:
/// * level: How important the message is.
/// * message: Text shown in the toast.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub level: NotifyLevel,
    pub message: String
}

/// The node toasts are stacked in, in the editor's bottom right corner.
#[derive(Debug, Clone, Copy, Component)]
pub struct ToastArea;

/// One shown toast, removed when `remaining` runs out or it is clicked.
///
/// Fields:
/// * remaining: Seconds left before the toast is removed.
#[derive(Debug, Clone, Copy, Component)]
pub struct Toast {
    pub remaining: f32
}

/// Builds the empty area toasts are stacked in.
///
/// It is fixed to the bottom right corner above everything else, and lets the pointer through
/// where there are no toasts.
pub(crate) fn toast_area() -> UiBundle {
    UiBundle::new(Node::column()
        .fixed(Rect::new(Val::Auto, px(16.0), Val::Auto, px(16.0)))
        .width(px(TOAST_WIDTH))
        .gap(px(8.0))
        .z_index(950))
        .with(ToastArea)
}

/// The accent color of a toast of `level`.
fn level_color(editor: &DeltosEditor, level: NotifyLevel) -> Vec4 {
    match level {
        NotifyLevel::Info => editor.theme.blue,
        NotifyLevel::Warning => editor.theme.yellow,
        NotifyLevel::Error => editor.theme.red,
    }
}

/// Builds the toast of one notification: its message beside a colored bar.
fn toast(editor: &DeltosEditor, notification: &Notification) -> UiBundle {
    let color = level_color(editor, notification.level);
    UiBundle::new(Node::row()
        .width(Val::Percent(1.0))
        .shrink(0.0)
        .background(editor.theme.bg_2)
        .border(1.0, color)
        .radius(px(5.0)))
        .with(Toast { remaining: TOAST_SECONDS })
        .with(HoverStyle::new(editor.theme.bg_3))
        .interactive()
        .child(UiBundle::new(Node::new()
            .width(px(4.0))
            .shrink(0.0)
            .background(color)))
        .child(UiBundle::new(Node::new()
            .grow(1.0)
            .min_width(px(0.0))
            .padding(Rect::single(px(10.0))))
            .with(themed::Label::new(notification.message.clone()).wrapped()))
}

/// Spawns a toast for every notification passed to `DeltosEditor::notify` since the last
/// frame, keeping at most `MAX_TOASTS`.  Notifications wait until the toast area is built.
fn show_toasts(world: World, mut editor: ResMut<DeltosEditor>) {
    if editor.notifications.is_empty() { return }
    let Some((area, _)) = Query::<&ToastArea>::new(&world).iter().next() else { return };

    for notification in std::mem::take(&mut editor.notifications) {
        world.spawn_with_parent(Some(area), toast(&editor, &notification).into_entity_builder());
    }

    // drop the oldest toasts beyond the limit
    let shown = world.hierarchy()
        .children(area)
        .map(|children| children.to_vec())
        .unwrap_or_default();
    for old in shown.iter().take(shown.len().saturating_sub(MAX_TOASTS)) {
        world.handle(*old).despawn_recursive();
    }
}

/// Counts down every toast's time and removes it when it runs out or is clicked.
fn expire_toasts(world: World, input: Res<UiInput>, delta: Res<RenderDeltaTime>) {
    let mut expired = Vec::new();
    let mut toasts = Query::<&mut Toast>::new(&world);
    while let Ok(Some((id, mut toast))) = toasts.next() {
        toast.remaining -= **delta;
        if toast.remaining <= 0.0 || input.activated(id) { expired.push(id) }
    }
    drop(toasts);

    for id in expired {
        world.handle(id).despawn_recursive();
    }
}
