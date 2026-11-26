use std::{fmt::Debug, sync::{Arc, Mutex, PoisonError}};

use anarchy::{Component, EntityID, Res, ResMut, Resource, World};
use gearbox::{Plugin, glam::Vec2, ui::{ComputedNode, Node, PositionType, Rect, UiInput, px}};
use uuid::Uuid;

use crate::{editor::NodeGraphEditor, tiles::NodeGraphTile};

pub mod editor;
pub mod tiles;

pub struct DeltosNodeGraphPlugin;
impl Plugin for DeltosNodeGraphPlugin {
    fn build(self, app: gearbox::App) -> gearbox::App {
        app
            .add_resource(TileDrag::default())
            .on_render_update(drag_tiles)
    }
}

/// Updates the stored position of a drawn tile.  Implemented for the editor shared by a
/// `NodeGraphFileEditor`, and used by `drag_tiles` when a tile is dropped.
pub trait TilePositions: Debug + Send + Sync {
    /// Calls `NodeGraphTile::set_position` on the tile with `tile_id`, if there is one.
    fn set_tile_position(&self, tile_id: Uuid, position: Vec2);
}

impl<Editor: NodeGraphEditor> TilePositions for Mutex<Editor> {
    fn set_tile_position(&self, tile_id: Uuid, position: Vec2) {
        let mut editor = self
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(tile) = editor
            .tiles_mut()
            .iter_mut()
            .find(|tile| tile.tile_id() == tile_id) {
            tile.set_position(position);
        }
    }
}

/// Marks the root UI node of a drawn tile so `drag_tiles` can move it.
///
/// Fields:
/// * tile_id: ID of the `NodeGraphTile` this node draws.
/// * positions: Editor the tile belongs to, told the tile's new position when it is dropped.
#[derive(Debug, Clone, Component)]
pub struct TileNode {
    pub tile_id: Uuid,
    pub positions: Arc<dyn TilePositions>
}

/// A tile held by the pointer.
///
/// Fields:
/// * entity: The held tile's root UI node.
/// * grab: Where on the tile the pointer pressed, from the tile's top left, in window pixels.
/// * offset: Where the tile was last moved to, in logical pixels from its parent's top left.
#[derive(Debug, Clone, Copy)]
pub struct HeldTile {
    pub entity: EntityID,
    pub grab: Vec2,
    pub offset: Option<Vec2>
}

/// The tile being dragged, if any.
///
/// Fields:
/// * 0: The held tile, `None` while no tile is pressed.
#[derive(Debug, Default, Resource)]
pub struct TileDrag(pub Option<HeldTile>);

/// Drags tiles around: pressing a tile picks it up, and it follows the pointer until let go.
/// Dropping a tile that moved stores its new position with `NodeGraphTile::set_position`.
fn drag_tiles(world: World, input: Res<UiInput>, mut drag: ResMut<TileDrag>) {
    // pick up a tile when it is pressed
    if input.pressed && let Some(id) = input.hovered && world.handle(id).get::<&TileNode>().ok().flatten().is_some() {
        let Some(position) = computed_position(&world, id) else { return };
        drag.0 = Some(HeldTile { entity: id, grab: input.cursor - position, offset: None });
    }
    let Some(mut held) = drag.0 else { return };

    // drop the tile, storing where it ended up
    if !input.down {
        drag.0 = None;
        let tile = world
            .handle(held.entity)
            .get::<&TileNode>()
            .ok()
            .flatten()
            .map(|tile| tile.clone());
        if let (Some(tile), Some(offset)) = (tile, held.offset) {
            tile.positions.set_tile_position(tile.tile_id, offset);
        }
        return;
    }

    // keep the grabbed spot of the tile under the pointer
    let parent = world
        .hierarchy()
        .parent(held.entity)
        .and_then(|parent| computed_position(&world, parent))
        .unwrap_or(Vec2::ZERO);
    let offset = (input.cursor - held.grab - parent) / input.scale.max(f32::EPSILON);
    if held.offset != Some(offset) {
        set_offset(&world, held.entity, offset);
        held.offset = Some(offset);
        drag.0 = Some(held);
    }
}

/// The laid out position of `entity`'s node in window pixels.
fn computed_position(world: &World, entity: EntityID) -> Option<Vec2> {
    world
        .handle(entity)
        .get::<&ComputedNode>()
        .ok()
        .flatten()
        .map(|computed| computed.position)
}

/// Places `entity`'s node at `offset` logical pixels from its parent's top left.
fn set_offset(world: &World, entity: EntityID, offset: Vec2) {
    let Ok(Some(mut node)) = world.handle(entity).get::<&mut Node>() else { return };
    node.style_mut().set_position_type(PositionType::Absolute(Rect::new_top_left(px(offset.y), px(offset.x))));
}
