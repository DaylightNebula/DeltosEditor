use std::fmt::Debug;

use anarchy::World;
use gearbox::ui::*;
use uuid::Uuid;

use crate::{DeltosEditor, SIDE_BAR_UUID_NAMESPACE};

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SideBarSlot {
    Left,
    Right
}

/// Render the left and right sidebars into a pair of `UiBundle`s.
pub fn render_left_right(world: &World, editor: &DeltosEditor) -> (UiBundle, UiBundle) {
    let left_content = editor.side_bars.values()
        .filter(|a| a.default_slot() == SideBarSlot::Left)
        .next()
        .map(|a| a.build(world, editor))
        .unwrap_or_else(|| UiBundle::new(Node::new()));
    let right_content = editor.side_bars.values()
        .filter(|a| a.default_slot() == SideBarSlot::Right)
        .next()
        .map(|a| a.build(world, editor))
        .unwrap_or_else(|| UiBundle::new(Node::new()));

    (left_content, right_content)
}
