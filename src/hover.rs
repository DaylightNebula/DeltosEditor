use anarchy::{Component, Query};
use gearbox::{glam::Vec4, ui::*, *};

/// Implements a general hover and click color changing system via the `HoverStyle` component.
pub struct HoverPlugin;
impl Plugin for HoverPlugin {
    fn build(self, app: App) -> App {
        app.on_render_update(apply_hover_styles)
    }
}

/// Marks a ui `Node` to change color when a user hovers or clicks on an item.
///
/// Fields:
/// * hover: Vec4 -> The hover color for this style
/// * normal: Optional Vec4 -> The base color of the element, defaults to a 0x00000000 color.
/// * pressed: Optional Vec4 -> The pressed color of the element, defaults to the hover color.
#[derive(Component, Clone, Copy, Debug)]
pub struct HoverStyle {
    pub hover: Vec4,
    pub normal: Option<Vec4>,
    pub pressed: Option<Vec4>
}

impl HoverStyle {
    /// Create a new `HoverStyle` instance.
    pub const fn new(hover: Vec4) -> Self { Self { hover, normal: None, pressed: None } }

    /// Sets the base color of a `HoverStyle`.
    pub const fn base(mut self, normal: Vec4) -> Self { self.normal = Some(normal); self }

    /// Sets the pressed color of a `HoverStyle`.
    pub const fn pressed(mut self, pressed: Vec4) -> Self { self.pressed = Some(pressed); self }

    /// Get the color of this `HoverStyle` relative to the given `Interaction`.
    pub fn color(&self, interaction: Interaction) -> Vec4 {
        match interaction {
            Interaction::None => self.normal.unwrap_or_else(|| Vec4::ZERO),
            Interaction::Hovered => self.hover,
            Interaction::Pressed => self.pressed.unwrap_or(self.hover),
        }
    }
}

fn apply_hover_styles(mut nodes: Query<(&'static HoverStyle, &'static Interaction, &'static mut Node)>) {
    while let Ok(Some((_, (hover, interaction, mut node)))) = nodes.next() {
        let wanted = hover.color(*interaction);
        let current = match node.style().background() {
            Background::Color(c) => *c,
            Background::Empty => Vec4::ZERO,
            _ => continue
        };

        // only write when it changes, so the style isn't touched every frame
        if current != wanted {
            node.style_mut().set_background(Background::Color(wanted));
        }
    }
}
