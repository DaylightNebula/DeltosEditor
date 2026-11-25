use std::fmt::Debug;

use deltos::theme::Theme;
use gearbox::glam::Vec4;
use getset::{CopyGetters, Getters};
use uuid::Uuid;

pub mod editor;

/// Implemented by anything that can be drawn as a single tile (node) in a node graph.  Returned
/// by `NodeGraphEditor::graph` and drawn by `NodeGraphFileEditor`.
pub trait NodeGraphTile: Debug + Send {
    /// Unique ID of this tile.
    fn tile_id(&self) -> Uuid;

    /// Text shown in the tile's title bar.
    fn title(&self) -> String;

    /// Background color of the tile's title bar.
    fn color(&self, theme: &Theme) -> Vec4;

    /// Inputs shown on the left side of the tile.
    fn inputs(&self, theme: &Theme) -> Vec<IOButton>;

    /// Outputs shown on the right side of the tile.
    fn outputs(&self, theme: &Theme) -> Vec<IOButton>;
}

/// An input or output connection point on a tile.
///
/// Fields:
/// * name: Label shown next to the connection point.
/// * color: Color of the connection point.
#[derive(Clone, Debug, Getters, CopyGetters)]
pub struct IOButton {
    #[getset(get = "pub")]
    name: String,
    #[getset(get_copy = "pub")]
    color: Vec4
}

impl IOButton {
    /// Creates an input or output connection point.
    ///
    /// # Parameters
    /// * name: Label shown next to the connection point.
    /// * color: Color of the connection point.
    pub fn new(name: impl Into<String>, color: Vec4) -> Self {
        Self { name: name.into(), color }
    }
}
