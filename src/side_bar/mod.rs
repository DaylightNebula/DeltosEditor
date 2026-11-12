use std::fmt::Debug;

use anarchy::World;
use gearbox::{ui::*};
use uuid::Uuid;

use crate::{DeltosEditor, SIDE_BAR_UUID_NAMESPACE, hover::HoverStyle};

pub mod file_selector;

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
/// * Left
/// * Right
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SideBarSlot {
    #[default]
    Left,
    Right
}

/// Render the left and right sidebars into a pair of `UiBundle`s.
pub fn render_left_right(world: &World, editor: &DeltosEditor) -> (UiBundle, UiBundle) {
    let (left_icons, right_icons): (Vec<_>, Vec<_>) = editor.side_bars.values()
        .map(|a| (a.default_slot(), a.id(), a.icon()))
        .partition(|(dir, _, _)| *dir == SideBarSlot::Left);

    let (left_id, left_content) = editor.side_bars.values()
        .filter(|a| a.default_slot() == SideBarSlot::Left)
        .next()
        .map(|a| (Some(a.id()), a.build(world, editor)))
        .unwrap_or_else(|| (None, UiBundle::new(Node::new())));
    let (right_id, right_content) = editor.side_bars.values()
        .filter(|a| a.default_slot() == SideBarSlot::Right)
        .next()
        .map(|a| (Some(a.id()), a.build(world, editor)))
        .unwrap_or_else(|| (None, UiBundle::new(Node::new())));

    let left = UiBundle::new(Node::row()
        .min_width(Val::Percent(0.25))
        .max_width(Val::Percent(0.25))
        .background(editor.theme.bg_1))
        .child(button_column(editor, left_icons, left_id))
        .child(left_content);

    let right = UiBundle::new(Node::row()
        .min_width(Val::Percent(0.25))
        .max_width(Val::Percent(0.25))
        .background(editor.theme.bg_1))
        .child(right_content)
        .child(button_column(editor, right_icons, right_id));

    (left, right)
}

fn button_column(editor: &DeltosEditor, icons: Vec<(SideBarSlot, Uuid, String)>, selected_id: Option<Uuid>) -> UiBundle {
    UiBundle::new(Node::column()
        .padding(Rect::new(Val::Px(5.0), Val::Px(5.0), Val::Px(5.0), Val::Px(5.0)))
        .gap(px(4.0))
        .min_width(Val::Px(35.0))
        .max_width(Val::Px(35.0))
        .background(editor.theme.bg_0))
        .children(
            icons.iter()
                .map(|(_slot, id, icon)|
                    UiBundle::new(Node::new()
                        .width(px(35.0))
                        .height(px(35.0))
                        .justify(JustifyContent::Center)
                        .align(AlignItems::Center)
                        .radius(px(5.0)))
                        .text(Text::new(editor.font.clone(), icon.to_string(), if Some(*id) == selected_id { editor.theme.blue } else { editor.theme.bg_8 }, 24.0)
                            .with_align(Align::Center, Align::Center))
                        .with(HoverStyle::new(editor.theme.bg_4))
                        .interactive()
                )
        )
}
