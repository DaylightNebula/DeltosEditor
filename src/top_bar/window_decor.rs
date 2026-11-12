use gearbox::ui::*;

use crate::{top_bar::{TopBarElement, caption_button}, window::WindowButton};

#[derive(Debug)]
pub struct TopWindowDecorElement;
impl TopBarElement for TopWindowDecorElement {
    fn name(&self) -> String {
        "Window Decor".to_string()
    }

    fn alignment(&self) -> gearbox::ui::Align {
        Align::End
    }

    fn build(&self, _world: &anarchy::World, editor: &crate::DeltosEditor) -> gearbox::ui::UiBundle {
        UiBundle::new(Node::row()
            .height(Val::Percent(1.0)))
            .children([
                caption_button(editor, "–", WindowButton::Minimize),
                caption_button(editor, "󱓼", WindowButton::Maximize),
                caption_button(editor, "×", WindowButton::Close)
            ])
    }
}
