use anarchy::{ResMut, SystemPin, World, pin};
use deltos::{DeltosEditor, DeltosPlugin, editors::text::SimpleTextDeltosEditorPlugin, side_bar::{SideBarElement, SideBarPlugin, SideBarSlot}, sources::local::LocalFileSource};
use gearbox::{App, anyhow, ui::{Align, Node, Rect, Text, UiBundle, Val, px}};

fn main() -> anyhow::Result<()> {
    App::new()
        .add_plugin(DeltosPlugin::default().with_source(LocalFileSource))
        .add_plugin(SimpleTextDeltosEditorPlugin)
        .add_plugin(SideBarPlugin)
        .on_startup_with(setup, pin(SystemPin::Start))
        .run()
}

/// A side bar that shows its own name, for trying out selecting, hiding and resizing side bars.
///
/// Fields:
/// * 0: Name shown in the side bar, also used to derive its ID.
/// * 1: Text based icon shown in the side bar's button column.
/// * 2: Which side bar it is added to.
#[derive(Debug)]
pub struct TestSideBarElement(&'static str, &'static str, SideBarSlot);
impl SideBarElement for TestSideBarElement {
    fn name(&self) -> String { self.0.to_string() }
    fn icon(&self) -> String { self.1.to_string() }
    fn default_slot(&self) -> SideBarSlot { self.2 }

    fn build(&self, _: &World, editor: &DeltosEditor) -> UiBundle {
        UiBundle::new(Node::new()
            .width(Val::Percent(1.0))
            .padding(Rect::new(px(8.0), px(8.0), px(12.0), px(12.0))))
            .text(Text::new(editor.font.clone(), self.0.to_uppercase(), editor.theme.bg_8, 12.0)
                .with_align(Align::Start, Align::Start))
    }
}

/// Adds two test side bars to each side of the editor.
fn setup(mut editor: ResMut<DeltosEditor>) {
    editor.add_side_bar(Box::new(TestSideBarElement("Test Search", "\u{ea6d}", SideBarSlot::Left)));
    editor.add_side_bar(Box::new(TestSideBarElement("Test Debug", "\u{eaaf}", SideBarSlot::Left)));
    editor.add_side_bar(Box::new(TestSideBarElement("Test Extensions", "\u{eae6}", SideBarSlot::Right)));
    editor.add_side_bar(Box::new(TestSideBarElement("Test Settings", "\u{eb51}", SideBarSlot::Right)));
}
