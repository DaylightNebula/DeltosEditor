use std::fmt::Debug;

use deltos::theme::Theme;
use derive_more::{Deref, DerefMut};
use gearbox::{anyhow, glam::{Vec2, Vec4}};
use getset::{CopyGetters, Getters};
use uuid::Uuid;

/// Implemented by anything that can be drawn as a single tile (node) in a node graph.  Returned
/// by `NodeGraphEditor::tiles` and drawn by `NodeGraphFileEditor`.
pub trait NodeGraphTile: Debug + Send {
    /// Unique ID of this tile.
    fn tile_id(&self) -> Uuid;

    /// Text shown in the tile's title bar.
    fn title(&self) -> String;

    /// Background color of the tile's title bar.
    fn color(&self, theme: &Theme) -> Vec4;

    /// The position of this tile, is the reposibility for each
    /// `NodeGraphTile` implementation to track position.
    fn position(&self) -> Vec2;

    /// Set the position of this tile.
    fn set_position(&mut self, position: Vec2);

    /// Connect an output button on this tile to another tiles input button.  Buttons are
    /// identified by `IOButton::id`.
    ///
    /// This may be cancelled to prevent any kind of illegal connections.
    fn connect(
        &mut self,
        output_button_id: Uuid,
        input_tile_id: Uuid,
        input_button_id: Uuid
    ) -> anyhow::Result<()>;

    /// Disconnect an output button on this tile from another tiles input button.  Buttons are
    /// identified by `IOButton::id`.
    fn disconnect(
        &mut self,
        output_button_id: Uuid,
        input_tile_id: Uuid,
        input_button_id: Uuid
    ) -> anyhow::Result<()>;

    /// Inputs shown on the left side of the tile.
    fn inputs(&self, theme: &Theme) -> Vec<IOButton>;

    /// Outputs shown on the right side of the tile.
    fn outputs(&self, theme: &Theme) -> Vec<(IOButton, Wires)>;

    /// Settings shown as text fields under the tile's inputs and outputs.  None by default.
    fn properties(&self) -> Vec<TileProperty> { Vec::new() }

    /// Sets the property named `name` from the text typed into its field.  Called when the field
    /// is submitted with Enter or by clicking elsewhere, after which the field shows the
    /// property's value from `properties` again.
    ///
    /// # Errors
    /// * The tile has no property named `name`, the default.
    /// * `value` isn't a valid value for the property.
    fn set_property(&mut self, name: &str, value: &str) -> anyhow::Result<()> {
        let _ = value;
        anyhow::bail!("{} has no property {name}", self.title())
    }
}

/// A setting of a tile, edited through a text field on the tile.
///
/// Fields:
/// * name: Identifies the property to `NodeGraphTile::set_property` and labels its field.  It
///   must stay the same every time the tile lists its properties.
/// * value: The property's current value, as shown in its field.
#[derive(Clone, Debug, PartialEq, Eq, Getters)]
pub struct TileProperty {
    #[getset(get = "pub")]
    name: String,
    #[getset(get = "pub")]
    value: String
}

impl TileProperty {
    /// Creates a property shown with its current value.
    ///
    /// # Parameters
    /// * name: Identifies the property and labels its field.
    /// * value: The property's current value.
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self { name: name.into(), value: value.into() }
    }
}

/// An input or output connection point on a tile.
///
/// Fields:
/// * id: Identifies the connection point to wires, `connect` and `disconnect`.  It must stay the
///   same every time the tile lists its inputs or outputs.
/// * name: Label shown next to the connection point.
/// * color: Color of the connection point.
/// * wire_rules: How many wires the connection point can have.
#[derive(Clone, Debug, Getters, CopyGetters)]
pub struct IOButton {
    #[getset(get_copy = "pub")]
    id: Uuid,
    #[getset(get = "pub")]
    name: String,
    #[getset(get_copy = "pub")]
    color: Vec4,
    #[getset(get_copy = "pub")]
    wire_rules: WireRules
}

impl IOButton {
    /// Creates an input or output connection point.
    ///
    /// # Parameters
    /// * id: Identifies the connection point, the same every time the tile lists it.
    /// * name: Label shown next to the connection point.
    /// * color: Color of the connection point.
    /// * wire_rules: How many wires the connection point can have.
    pub fn new(id: Uuid, name: impl Into<String>, color: Vec4, wire_rules: WireRules) -> Self {
        Self { id, name: name.into(), color, wire_rules }
    }
}

/// Defines if a wire node can take multiple wires in or out.
///
/// Variants
/// * Single: Single input or output wire
/// * Multi: Any amount of input or output wires
/// * Capped: Any amount of input or output wires capped at some amount
#[derive(Clone, Copy, Debug)]
pub enum WireRules { Single, Multi, Capped(usize) }

/// A single wire from an output to an input on another tile.
///
/// Fields:
/// * target_tile: ID of the tile the wire ends on.
/// * target_input: ID of the input on `target_tile` the wire ends at.
#[derive(CopyGetters, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wire {
    #[getset(get_copy = "pub")]
    target_tile: Uuid,
    #[getset(get_copy = "pub")]
    target_input: Uuid
}

impl Wire {
    /// Creates a wire to an input on another tile.
    ///
    /// # Parameters
    /// * target_tile: ID of the tile the wire ends on.
    /// * target_input: ID of the input on `target_tile` the wire ends at.
    pub fn new(target_tile: Uuid, target_input: Uuid) -> Self {
        Self { target_tile, target_input }
    }
}

/// The wires out of one output, each drawn as a line to its target input.
///
/// Fields:
/// * wires: The wires out of the output.
#[derive(Clone, Debug, Default, Deref, DerefMut)]
pub struct Wires {
    wires: Vec<Wire>
}

impl Wires {
    /// Creates the wires out of an output.
    ///
    /// # Parameters
    /// * wires: The wires out of the output.
    pub fn new(wires: Vec<Wire>) -> Self {
        Self { wires }
    }
}
