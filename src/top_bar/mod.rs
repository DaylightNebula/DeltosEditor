use std::fmt::Debug;

use gearbox::glam::Vec4;
use uuid::Uuid;

use crate::{hover::HoverStyle, window::{DragRegion, WindowButton}, *};

pub mod title_decor;
pub mod window_decor;

/// An element that can be placed in the top bar of the editor UI.
pub trait TopBarElement: Debug + Send {
    /// The UUID of this element derived from the TopBarElement name.
    fn id(&self) -> Uuid { Uuid::new_v5(&TOP_BAR_UUID_NAMESPACE, self.name().as_bytes()) }

    /// The name of this element.
    fn name(&self) -> String;

    /// Should this element be placed at the start, end, or center of the top bar
    fn alignment(&self) -> Align;

    /// The build a UiBundle for this element from a `World` and `DeltosEditor`.
    fn build(&self, world: &World, editor: &DeltosEditor) -> UiBundle;
}

pub(crate) fn render(world: &World, editor: &DeltosEditor) -> UiBundle {
    let mut start_bundles = Vec::new();
    let mut center_bundles = Vec::new();
    let mut end_bundles = Vec::new();

    center_bundles.push(center_spacer());

    for element in editor.top_bars.values() {
        match element.alignment() {
            Align::Start => start_bundles.push(element.build(world, editor)),
            Align::Center => {
                center_bundles.push(element.build(world, editor));
                center_bundles.push(center_spacer());
            },
            Align::End => end_bundles.push(element.build(world, editor)),
        }
    }

    UiBundle::new(Node::row()
        .width(Val::Percent(1.0))
        .height(px(40.0))
        .background(editor.theme.bg_1)
        .align(AlignItems::Center)
        .shrink(0.0))
        .child(
            UiBundle::new(Node::row()
                .align(AlignItems::Center)
                .height(Val::Percent(1.0)))
                .children(start_bundles)
        )
        .children(center_bundles)
        .child(
            UiBundle::new(Node::row()
                .align(AlignItems::Center)
                .height(Val::Percent(1.0)))
                .children(end_bundles)
        )
}

pub(crate) fn caption_button(editor: &DeltosEditor, label: &str, button: WindowButton) -> UiBundle{
    UiBundle::new(Node::row()
        .width(px(46.0))
        .height(Val::Percent(1.0))
        .align(AlignItems::Center)
        .justify(JustifyContent::Center)
        .shrink(0.0))
        .with(button)
        .with(HoverStyle::new(if button == WindowButton::Close { editor.theme.red } else { editor.theme.bg_4 }))
        .interactive()
        .text(Text::new(editor.font.clone(), label, Vec4::ONE, 16.0)
            .with_align(Align::Center, Align::Center))
}

fn center_spacer() -> UiBundle {
    UiBundle::new(Node::new()
        .grow(1.0)
        .min_width(px(0.0))
        .height(Val::Percent(1.0)))
        .with(DragRegion)
        .interactive()
}
