use gearbox::{glam::Vec4, ui::*};

use crate::{hover::HoverStyle, top_bar::TopBarElement};

#[derive(Debug)]
pub struct TitleDecorElement;
impl TopBarElement for TitleDecorElement {
    fn name(&self) -> String {
        "Title Decor".to_string()
    }

    fn alignment(&self) -> gearbox::ui::Align {
        gearbox::ui::Align::Start
    }

    fn build(&self, _world: &anarchy::World, editor: &crate::DeltosEditor) -> gearbox::ui::UiBundle {
        UiBundle::new(Node::row()
            .height(Val::Percent(1.0)))
            .child(
                UiBundle::new(Node::new()
                    .align(AlignItems::Center)
                    .height(Val::Percent(1.0))
                    .padding(Rect::new(px(10.0), px(10.0), px(14.0), px(14.0))))
                    .with(HoverStyle::new(editor.theme.bg_4))
                    .interactive()
                    .text(Text::new(editor.font.clone(), "".to_string(), Vec4::ONE, 18.0)
                        .with_align(Align::Center, Align::Center))
            )
            .child(
                UiBundle::new(Node::new()
                    .align(AlignItems::Center)
                    .height(Val::Percent(1.0))
                    .padding(Rect::new(px(10.0), px(10.0), px(14.0), px(14.0))))
                    .text(Text::new(editor.font.clone(), editor.title.clone(), Vec4::ONE, 14.0)
                        .with_align(Align::Center, Align::Center))
            )
    }
}
