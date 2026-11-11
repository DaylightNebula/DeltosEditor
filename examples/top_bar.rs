use anarchy::{ResMut, SystemPin, World, pin};
use deltos::{DeltosEditor, DeltosPlugin, editors::text::SimpleTextDeltosEditorPlugin, sources::local::LocalFileSource, top_bar::TopBarElement};
use gearbox::{App, anyhow, ui::{Align, Node, Text, UiBundle}};

fn main() -> anyhow::Result<()> {
    App::new()
        .add_plugin(DeltosPlugin::default().with_source(LocalFileSource))
        .add_plugin(SimpleTextDeltosEditorPlugin)
        .on_startup_with(setup, pin(SystemPin::Start))
        .run()
}

#[derive(Debug)]
pub struct TestTopBarElement(Align);
impl TopBarElement for TestTopBarElement {
    fn name(&self) -> String {
        "TEST TOP BAR".to_string()
    }

    fn alignment(&self) -> Align {
        self.0
    }

    fn build(&self, _: &World, editor: &DeltosEditor) -> UiBundle {
        UiBundle::new(Node::new()
            .text(Text::new(editor.font.clone(), "TEST TOP BAR", editor.theme.bg_8, 12.0)))
    }
}

fn setup(mut editor: ResMut<DeltosEditor>) {
    editor.add_top_bar(Box::new(TestTopBarElement(Align::Start)));
    editor.add_top_bar(Box::new(TestTopBarElement(Align::Center)));
    editor.add_top_bar(Box::new(TestTopBarElement(Align::End)));
}
