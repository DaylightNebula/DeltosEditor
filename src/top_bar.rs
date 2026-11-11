use std::fmt::Debug;

use gearbox::glam::Vec4;
use uuid::Uuid;

use crate::{hover::HoverStyle, window::{DragRegion, WindowButton}, *};

/// An element that can be placed in the top bar of the editor UI.
pub trait TopBarElement: Debug + Send {
    /// The UUID of this element derived from the TopBarElement name.
    fn id(&self) -> Uuid { Uuid::new_v5(&TOP_BAR_UUID_NAMESPACE, self.name().as_bytes()) }

    /// The name of this element.
    fn name(&self) -> String;

    /// The build a UiBundle for this element from a `World` and `DeltosEditor`.
    fn build(&self, world: &World, editor: &DeltosEditor) -> UiBundle;
}

pub(crate) fn render(world: &World, editor: &DeltosEditor) -> UiBundle {
    let mut mid_children = Vec::new();
    mid_children.push(center_spacer());

    for top_bar in editor.top_bars.values() {
        mid_children.push(top_bar.build(world, editor));
        mid_children.push(center_spacer());
    }

    UiBundle::new(Node::row()
        .width(Val::Percent(1.0))
        .height(px(40.0))
        .background(editor.theme.bg_1)
        .align(AlignItems::Center)
        .shrink(0.0))
        .child(
            UiBundle::new(Node::row())
                .child(
                    UiBundle::new(Node::row()
                        .align(AlignItems::Center)
                        .height(Val::Percent(1.0))
                        .padding(Rect::new(px(10.0), px(10.0), px(14.0), px(14.0))))
                        .with(HoverStyle::new(editor.theme.bg_4))
                        .interactive()
                        .text(Text::new(editor.font.clone(), "".to_string(), Vec4::ONE, 18.0)
                            .with_align(Align::Center, Align::Center))
                )
                .child(
                    UiBundle::new(Node::row()
                        .align(AlignItems::Center)
                        .height(Val::Percent(1.0))
                        .padding(Rect::new(px(10.0), px(10.0), px(14.0), px(14.0))))
                        .text(Text::new(editor.font.clone(), editor.title.clone(), Vec4::ONE, 14.0)
                            .with_align(Align::Center, Align::Center))
                )
        )
        .children(mid_children)
        .children([
            caption_button(editor, "–", WindowButton::Minimize),
            caption_button(editor, "󱓼", WindowButton::Maximize),
            caption_button(editor, "×", WindowButton::Close),
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
