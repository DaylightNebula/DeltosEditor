use std::{collections::{HashMap, HashSet}, fmt::Debug, sync::{Arc, Mutex, MutexGuard, PoisonError}};

use anarchy::{Component, ComponentMeta, EntityID, Event, Query, Res, ResMut, Resource, World};
use deltos::{DeltosEditor, editors::text::{OpenFile, save_target}, notify::NotifyLevel, sources::{TaskResult, is_conflict}, theme::Theme};
use gearbox::{Plugin, anyhow::{self, Context}, glam::{Vec2, Vec4}, ui::{Activate, ComputedNode, ContextMenu, Display, EntityUiElements, Node, PositionType, Rect, SDFFont, Submit, TextInput, UiBundle, UiFocus, UiInput, Val, px}};
use uuid::Uuid;
use winit::keyboard::{Key, NamedKey};

use crate::{editor::NodeGraphEditor, tiles::{NodeGraphTile, TileProperty, WireRules}};

pub mod editor;
pub mod tiles;

/// Adds the systems that drag tiles, highlight hovered input and output dots, connect dots with
/// wires, disconnect wires, pan graphs, add tiles from a graph's right click menu, delete tiles,
/// edit tile properties, and save edited graphs.  Graphs opened read only can't be changed.
pub struct DeltosNodeGraphPlugin;
impl Plugin for DeltosNodeGraphPlugin {
    fn build(self, app: gearbox::App) -> gearbox::App {
        app
            .add_resource(TileDrag::default())
            .add_resource(PendingWire::default())
            .add_resource(GraphPan::default())
            .add_resource(NewTileMenu::default())
            .on_render_update(drag_tiles)
            .on_render_update(hover_rings)
            .on_render_update(connect_wires)
            .on_render_update(disconnect_wires)
            .on_render_update(pan_graphs)
            .on_render_update(add_tiles)
            .on_render_update(delete_tiles)
            .on_render_update(edit_properties)
            .on_render_update(save_graphs)
            .on_render_update(draw_wires)
    }
}

/// A node graph editor shared between a `NodeGraphFileEditor` and the UI it built, so the
/// systems in this crate can change the graph.  Implemented for the editor's `Mutex`.
pub trait SharedGraph: Debug + Send + Sync {
    /// Calls `NodeGraphTile::set_position` on the tile with `tile_id`, if there is one.  Used by
    /// `drag_tiles` when a tile is dropped.
    fn set_tile_position(&self, tile_id: Uuid, position: Vec2);

    /// Calls `NodeGraphTile::connect` on the output's tile.  Used by `connect_wires` when a wire
    /// is finished.
    fn connect(&self, output_tile: Uuid, output_button: Uuid, input_tile: Uuid, input_button: Uuid) -> anyhow::Result<()>;

    /// Calls `NodeGraphTile::disconnect` on the output's tile.  Used by `connect_wires` to replace
    /// the wire of a full `WireRules::Single` end.
    fn disconnect(&self, output_tile: Uuid, output_button: Uuid, input_tile: Uuid, input_button: Uuid) -> anyhow::Result<()>;

    /// The wires out of every tile's outputs, drawn by `draw_wires`.
    fn wires(&self, theme: &Theme) -> Vec<WireNode>;

    /// Calls `NodeGraphEditor::new_tile` and builds the tiles it added, placed moved by `offset`,
    /// the graph's pan.  Used by `add_tiles` when a new tile is chosen.
    fn add_tile(&self, name: String, position: Vec2, offset: Vec2, theme: &Theme, font: &Arc<SDFFont>) -> Vec<UiBundle>;

    /// Disconnects every wire into the tile with `tile_id`, then calls
    /// `NodeGraphEditor::remove_tile`.  Used by `delete_tiles`.
    ///
    /// # Errors
    /// * There's no tile with `tile_id` in the graph.
    /// * A wire into the tile can't be disconnected, or the tile can't be removed.
    fn remove_tile(&self, tile_id: Uuid, theme: &Theme) -> anyhow::Result<()>;

    /// Calls `NodeGraphTile::set_property` on the tile with `tile_id`.  Used by `edit_properties`
    /// when a property field is submitted.
    ///
    /// # Errors
    /// * There's no tile with `tile_id` in the graph.
    /// * The tile refuses the value.
    fn set_property(&self, tile_id: Uuid, name: &str, value: &str) -> anyhow::Result<()>;

    /// The properties of the tile with `tile_id`, empty if there's no such tile.  Used by
    /// `edit_properties` to show a property's value after its field is submitted.
    fn properties(&self, tile_id: Uuid) -> Vec<TileProperty>;

    /// Calls `NodeGraphEditor::save`.  Used by `save_graphs`.
    ///
    /// # Errors
    /// * The graph can't be written.
    fn save(&self) -> anyhow::Result<String>;
}

impl<Editor: NodeGraphEditor> SharedGraph for Mutex<Editor> {
    fn set_tile_position(&self, tile_id: Uuid, position: Vec2) {
        if let Some(tile) = lock(self)
            .tiles_mut()
            .iter_mut()
            .find(|tile| tile.tile_id() == tile_id) {
            tile.set_position(position);
        }
    }

    fn connect(&self, output_tile: Uuid, output_button: Uuid, input_tile: Uuid, input_button: Uuid) -> anyhow::Result<()> {
        lock(self)
            .tiles_mut()
            .iter_mut()
            .find(|tile| tile.tile_id() == output_tile)
            .with_context(|| format!("No tile {output_tile} in the graph"))?
            .connect(output_button, input_tile, input_button)
    }

    fn disconnect(&self, output_tile: Uuid, output_button: Uuid, input_tile: Uuid, input_button: Uuid) -> anyhow::Result<()> {
        lock(self)
            .tiles_mut()
            .iter_mut()
            .find(|tile| tile.tile_id() == output_tile)
            .with_context(|| format!("No tile {output_tile} in the graph"))?
            .disconnect(output_button, input_tile, input_button)
    }

    fn wires(&self, theme: &Theme) -> Vec<WireNode> { editor::wire_nodes(&*lock(self), theme) }

    fn add_tile(&self, name: String, position: Vec2, offset: Vec2, theme: &Theme, font: &Arc<SDFFont>) -> Vec<UiBundle> {
        let mut editor = lock(self);
        let before: HashSet<Uuid> = editor
            .tiles()
            .iter()
            .map(|tile| tile.tile_id())
            .collect();
        editor.new_tile(name, position);

        // build only the tiles that weren't there before
        editor
            .tiles()
            .iter()
            .filter(|tile| !before.contains(&tile.tile_id()))
            .map(|tile| editor::tile_bundle(theme, font, tile, offset, false))
            .collect()
    }

    fn remove_tile(&self, tile_id: Uuid, theme: &Theme) -> anyhow::Result<()> {
        let mut editor = lock(self);
        anyhow::ensure!(editor.tiles().iter().any(|tile| tile.tile_id() == tile_id), "No tile {tile_id} in the graph");

        // disconnect the wires other tiles run into it, then remove it with the wires out of it
        let into_tile = editor::wire_nodes(&*editor, theme)
            .into_iter()
            .filter(|wire| wire.input_tile == tile_id && wire.output_tile != tile_id);
        for wire in into_tile {
            editor
                .tiles_mut()
                .iter_mut()
                .find(|tile| tile.tile_id() == wire.output_tile)
                .with_context(|| format!("No tile {} in the graph", wire.output_tile))?
                .disconnect(wire.output_button, wire.input_tile, wire.input_button)?;
        }
        editor.remove_tile(tile_id)
    }

    fn set_property(&self, tile_id: Uuid, name: &str, value: &str) -> anyhow::Result<()> {
        lock(self)
            .tiles_mut()
            .iter_mut()
            .find(|tile| tile.tile_id() == tile_id)
            .with_context(|| format!("No tile {tile_id} in the graph"))?
            .set_property(name, value)
    }

    fn properties(&self, tile_id: Uuid) -> Vec<TileProperty> {
        lock(self)
            .tiles()
            .iter()
            .find(|tile| tile.tile_id() == tile_id)
            .map(NodeGraphTile::properties)
            .unwrap_or_default()
    }

    fn save(&self) -> anyhow::Result<String> { lock(self).save() }
}

/// Locks a shared editor, still using it if a panic poisoned the lock.
pub(crate) fn lock<Editor>(editor: &Mutex<Editor>) -> MutexGuard<'_, Editor> {
    editor
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

/// Marks the root UI node of a drawn graph.
///
/// Fields:
/// * graph: The graph this node draws.
/// * pan: How far the graph's background was dragged, in logical pixels.  Tiles are drawn moved
///   by it, and positions given to the graph have it taken away.
#[derive(Debug, Clone, Component)]
pub struct GraphRoot {
    pub graph: Arc<dyn SharedGraph>,
    pub pan: Vec2
}

/// Marks an item of a graph's new tile menu.
///
/// Fields:
/// * name: The option from `NodeGraphEditor::new_tile_options` the item adds.
#[derive(Debug, Clone, Component)]
pub struct NewTileItem {
    pub name: String
}

/// Marks the root UI node of a drawn tile so `drag_tiles` can move it.
///
/// Fields:
/// * tile_id: ID of the `NodeGraphTile` this node draws.
#[derive(Debug, Clone, Copy, Component)]
pub struct TileNode {
    pub tile_id: Uuid
}

/// Marks the dot of an input or output, which wires are drawn between.
///
/// Fields:
/// * tile_id: ID of the tile the dot is on.
/// * button_id: `IOButton::id` of the input or output, which identifies it.
/// * name: Name of the input or output, only used in messages.
/// * output: Whether the dot is an output rather than an input.
/// * color: Color of the dot.
/// * rules: How many wires the input or output can have, kept by `connect_wires`.
#[derive(Debug, Clone, Component)]
pub struct IODot {
    pub tile_id: Uuid,
    pub button_id: Uuid,
    pub name: String,
    pub output: bool,
    pub color: Vec4,
    pub rules: WireRules
}

/// A wire drawn from an output to an input, a child of its graph's `GraphRoot`.
///
/// Fields:
/// * output_tile: ID of the tile the wire starts on.
/// * output_button: `IOButton::id` of the output the wire starts at.
/// * input_tile: ID of the tile the wire ends on.
/// * input_button: `IOButton::id` of the input the wire ends at.
/// * color: Color of the wire, the output's color.
/// * drawn: The ends the wire was last drawn between, in logical pixels from the graph's top
///   left, `None` while either end isn't shown.
/// * hovered: Whether the wire was last drawn hovered, which draws it wider.
#[derive(Debug, Clone, Component)]
pub struct WireNode {
    pub output_tile: Uuid,
    pub output_button: Uuid,
    pub input_tile: Uuid,
    pub input_button: Uuid,
    pub color: Vec4,
    pub drawn: Option<(Vec2, Vec2)>,
    pub hovered: bool
}

/// Marks the text field of a tile's property.
///
/// Fields:
/// * tile_id: ID of the tile the property is on.
/// * name: `TileProperty::name` of the property, which identifies it.
#[derive(Debug, Clone, Component)]
pub struct PropertyField {
    pub tile_id: Uuid,
    pub name: String
}

/// Marks the ring shown around the hovered dot of a graph.
#[derive(Debug, Clone, Copy, Component)]
pub struct HoverRing;

/// Marks the wire drawn from a clicked dot to the pointer, before the wire is finished.
#[derive(Debug, Clone, Copy, Component)]
pub struct WirePreview;

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

/// The graph whose background is being dragged, if any.
///
/// Fields:
/// * 0: The dragged `GraphRoot` node, `None` while no background is pressed.
#[derive(Debug, Default, Resource)]
pub struct GraphPan(pub Option<EntityID>);

/// Where the new tile menu was last opened, which is where a chosen tile is added.
///
/// Fields:
/// * 0: Where the background was right clicked, in window pixels.
#[derive(Debug, Default, Resource)]
pub struct NewTileMenu(pub Option<Vec2>);

/// The dot a wire was started from, if any.
///
/// Fields:
/// * 0: The clicked `IODot` node, `None` while no wire is being made.
#[derive(Debug, Default, Resource)]
pub struct PendingWire(pub Option<EntityID>);

/// Drags tiles around: pressing a tile picks it up, and it follows the pointer until let go.
/// Dropping a tile that moved stores its new position with `NodeGraphTile::set_position`.
fn drag_tiles(world: World, input: Res<UiInput>, mut drag: ResMut<TileDrag>) {
    // pick up a tile when it is pressed
    if input.pressed && let Some(id) = input.hovered && has::<TileNode>(&world, id)
        && root_of(&world, id).is_some_and(|root| editable(&world, root)) {
        let Some((position, _)) = computed_rect(&world, id) else { return };
        drag.0 = Some(HeldTile { entity: id, grab: input.cursor - position, offset: None });
    }
    let Some(mut held) = drag.0 else { return };

    // drop the tile, storing where it ended up
    if !input.down {
        drag.0 = None;
        let tile = component::<TileNode>(&world, held.entity);
        let root = root_of(&world, held.entity);
        let graph = root.and_then(|root| component::<GraphRoot>(&world, root));
        if let (Some(tile), Some(root), Some(graph), Some(offset)) = (tile, root, graph, held.offset) {
            graph.graph.set_tile_position(tile.tile_id, offset - graph.pan);
            mark_dirty(&world, root);
        }
        return;
    }

    // keep the grabbed spot of the tile under the pointer
    let parent = world
        .hierarchy()
        .parent(held.entity)
        .and_then(|parent| computed_rect(&world, parent))
        .map_or(Vec2::ZERO, |(position, _)| position);
    let offset = (input.cursor - held.grab - parent) / scale(&input);
    if held.offset != Some(offset) {
        if let Ok(Some(mut node)) = world.handle(held.entity).get::<&mut Node>() {
            set_top_left(&mut node, offset);
        }
        held.offset = Some(offset);
        drag.0 = Some(held);
    }
}

/// Shows a second circle around the dot under the pointer, in the dot's color.
fn hover_rings(world: World, input: Res<UiInput>) {
    // find the hovered dot and where it is in its graph
    let hovered = input.hovered.and_then(|id| {
        let dot = component::<IODot>(&world, id)?;
        let root = root_of(&world, id)?;
        Some((root, center_in(&world, root, id, scale(&input))?, dot.color))
    });

    // show the ring of the graph with the hovered dot, and hide the others
    for (ring, root) in children_with::<HoverRing>(&world) {
        let Ok(Some(mut node)) = world.handle(ring).get::<&mut Node>() else { continue };
        match hovered {
            Some((hovered_root, center, color)) if hovered_root == root => {
                set_top_left(&mut node, center - Vec2::splat(editor::RING_SIZE / 2.0));
                if *node.style().border_color() != Some(color) {
                    node.style_mut().set_border_color(Some(color));
                }
                set_shown(&mut node, true);
            }
            _ => set_shown(&mut node, false)
        }
    }
}

/// Connects dots with wires: clicking one dot starts a wire, and clicking a dot on the other
/// side of a tile finishes it with `NodeGraphTile::connect`.  A full `WireRules::Single` end
/// lets go of its wire to make room, and a full `WireRules::Capped` end refuses the new wire.  Clicking the same dot again or
/// anywhere off a dot drops the wire.  While a wire is started, a preview of it follows the
/// pointer, snapping to a dot that can finish it.
fn connect_wires(world: World, input: Res<UiInput>, editor: Res<DeltosEditor>, mut pending: ResMut<PendingWire>) {
    // a press anywhere but a dot drops the started wire
    let hovered_dot = input.hovered.filter(|id| has::<IODot>(&world, *id));
    if input.pressed && hovered_dot.is_none() { pending.0 = None }

    // clicking a dot starts a wire, or finishes the started one
    if let Some(clicked) = hovered_dot.filter(|id| input.activated(*id)) {
        pending.0 = match pending.0 {
            Some(start) if start == clicked => None,
            Some(start) if finish_wire(&world, &editor.theme, start, clicked) => None,
            _ => Some(clicked)
        };
    }

    draw_preview(&world, &input, pending.0, hovered_dot);
}

/// Connects the output and input of `a` and `b`, then redraws the wires of every view of their
/// graph.  Returns false without connecting if they aren't an output and an input of one graph.
/// Wires that already exist aren't connected again.  To keep both ends within their `WireRules`,
/// a full `Single` end has its wire disconnected first, and a full `Capped` end refuses.
fn finish_wire(world: &World, theme: &Theme, a: EntityID, b: EntityID) -> bool {
    let (Some(dot_a), Some(dot_b)) = (component::<IODot>(world, a), component::<IODot>(world, b)) else { return false };
    let Some(root) = root_of(world, a) else { return false };
    if dot_a.output == dot_b.output || root_of(world, b) != Some(root) { return false }
    let Some(GraphRoot { graph, .. }) = component::<GraphRoot>(world, root) else { return false };
    if !editable(world, root) { return true }

    // skip wires that exist already
    let (output, input) = if dot_a.output { (dot_a, dot_b) } else { (dot_b, dot_a) };
    let current = graph.wires(theme);
    let from_output = |wire: &&WireNode| wire.output_tile == output.tile_id && wire.output_button == output.button_id;
    let into_input = |wire: &&WireNode| wire.input_tile == input.tile_id && wire.input_button == input.button_id;
    if current.iter().any(|wire| from_output(&wire) && into_input(&wire)) { return true }

    // full single ends give up their wire, other full ends refuse the new one
    let ends: [(WireRules, Vec<&WireNode>); 2] = [
        (output.rules, current.iter().filter(from_output).collect()),
        (input.rules, current.iter().filter(into_input).collect())
    ];
    let mut replaced = Vec::new();
    for (rules, wires) in ends {
        if allows_another(rules, wires.len()) { continue }
        if !matches!(rules, WireRules::Single) {
            eprintln!("Can't connect {} to {}: it would break their wire rules", output.name, input.name);
            return true;
        }
        replaced.extend(wires);
    }

    // disconnect the replaced wires, then connect through the output's tile
    let disconnected = replaced
        .iter()
        .try_for_each(|wire| graph.disconnect(wire.output_tile, wire.output_button, wire.input_tile, wire.input_button));
    let result = disconnected.and_then(|_| graph.connect(output.tile_id, output.button_id, input.tile_id, input.button_id));
    if let Err(err) = result {
        eprintln!("Failed to connect {} to {}: {err:#}", output.name, input.name);

        // the graph is unchanged if nothing was disconnected, otherwise its wires still need redrawing
        if replaced.is_empty() { return true }
    }

    respawn_wires(world, theme, &graph);
    mark_dirty(world, root);
    true
}

/// Disconnects the wire under the pointer when it is right clicked, which is also a two finger
/// tap on most trackpads.  Wires are drawn behind the tiles, so clicks over a tile are left alone.
fn disconnect_wires(world: World, input: Res<UiInput>, layout: Res<EntityUiElements>, editor: Res<DeltosEditor>) {
    if !input.right_pressed { return }
    let Some(id) = wire_under_pointer(&world, &input, &layout) else { return };
    let Some(wire) = component::<WireNode>(&world, id) else { return };
    let Some(root) = world.hierarchy().parent(id) else { return };
    let Some(GraphRoot { graph, .. }) = component::<GraphRoot>(&world, root) else { return };
    if !editable(&world, root) { return }

    // disconnect it and redraw the graph's wires
    if let Err(err) = graph.disconnect(wire.output_tile, wire.output_button, wire.input_tile, wire.input_button) {
        eprintln!("Failed to disconnect output {} from input {}: {err:#}", wire.output_button, wire.input_button);
        return;
    }
    respawn_wires(&world, &editor.theme, &graph);
    mark_dirty(&world, root);
}

/// The wire closest to the pointer, within `editor::WIRE_HIT_DISTANCE` of it, in the shown graph
/// under the pointer.  Wires are drawn behind the tiles, so there's none while the pointer is over
/// a tile or dot.
fn wire_under_pointer(world: &World, input: &UiInput, layout: &EntityUiElements) -> Option<EntityID> {
    if input.hovered.is_some_and(|id| !has::<GraphRoot>(world, id)) { return None }
    let scale = scale(input);

    let wires: Vec<(EntityID, WireNode)> = Query::<&WireNode>::new(world)
        .iter()
        .map(|(id, wire)| (id, wire.clone()))
        .collect();
    let mut closest: Option<(f32, EntityID)> = None;
    for (id, wire) in wires {
        // only wires drawn in a graph shown under the pointer
        let Some((start, end)) = wire.drawn else { continue };
        let Some(root) = world.hierarchy().parent(id) else { continue };
        let Ok(Some(computed)) = world.handle(root).get::<&ComputedNode>() else { continue };
        if computed.pass != layout.pass || !computed.contains(input.cursor) { continue }

        let distance = editor::wire_distance(start, end, (input.cursor - computed.position) / scale);
        if distance <= editor::WIRE_HIT_DISTANCE && closest.is_none_or(|(best, _)| distance < best) {
            closest = Some((distance, id));
        }
    }
    closest.map(|(_, id)| id)
}

/// Pans a graph: pressing its background and dragging moves every tile with the pointer.  Wires,
/// the hover ring and the wire preview follow the dots, so they move along.
fn pan_graphs(world: World, input: Res<UiInput>, mut pan: ResMut<GraphPan>) {
    // grab a graph when its background is pressed
    if input.pressed && let Some(id) = input.hovered && has::<GraphRoot>(&world, id) {
        pan.0 = Some(id);
    }
    let Some(root) = pan.0 else { return };
    if !input.down {
        pan.0 = None;
        return;
    }
    let delta = input.delta / scale(&input);
    if delta == Vec2::ZERO { return }

    // move the view, then the tiles with it
    if let Ok(Some(mut graph)) = world.handle(root).get::<&mut GraphRoot>() {
        graph.pan += delta;
    }
    let tiles: Vec<EntityID> = children_with::<TileNode>(&world)
        .into_iter()
        .filter(|(_, parent)| *parent == root)
        .map(|(id, _)| id)
        .collect();
    for tile in tiles {
        let Ok(Some(mut node)) = world.handle(tile).get::<&mut Node>() else { continue };
        if let PositionType::Absolute(edges) = node.style().position_type()
            && let (Val::Px(left), Val::Px(top)) = (*edges.left(), *edges.top()) {
            set_top_left(&mut node, Vec2::new(left, top) + delta);
        }
    }
}

/// Adds tiles from a graph's right click menu.  The menu only opens over the graph's empty
/// background, so right clicking a wire still disconnects it, and a chosen tile is added where
/// the menu was opened.
fn add_tiles(
    world: World,
    input: Res<UiInput>,
    layout: Res<EntityUiElements>,
    editor: Res<DeltosEditor>,
    activated: Event<Activate>,
    mut opened: ResMut<NewTileMenu>
) {
    // allow the menu only over the background of the hovered graph, away from its wires
    let over_wire = wire_under_pointer(&world, &input, &layout).is_some();
    let roots: Vec<EntityID> = Query::<&GraphRoot>::new(&world)
        .iter()
        .map(|(id, _)| id)
        .collect();
    for root in roots {
        let disabled = over_wire || input.hovered != Some(root) || !editable(&world, root);
        if let Ok(Some(mut menu)) = world.handle(root).get::<&mut ContextMenu>()
            && menu.disabled != disabled {
            menu.disabled = disabled;
        }
        if input.right_pressed && !disabled { opened.0 = Some(input.cursor) }
    }

    // add the chosen tile where the menu opened, without the graph's pan
    let scale = scale(&input);
    for event in activated.read() {
        let entity = event.entity;
        let Some(item) = component::<NewTileItem>(&world, entity) else { continue };
        let Some(root) = root_of(&world, entity) else { continue };
        let Some(GraphRoot { graph, pan }) = component::<GraphRoot>(&world, root) else { continue };
        let (Some(at), Some((root_position, _))) = (opened.0, computed_rect(&world, root)) else { continue };

        let position = (at - root_position) / scale - pan;
        for tile in graph.add_tile(item.name, position, pan, &editor.theme, &editor.font) {
            world.spawn_with_parent(Some(root), tile.into_entity_builder());
        }
        mark_dirty(&world, root);
    }
}

/// Deletes the tile under the pointer when Delete or Backspace is pressed, along with every wire
/// into or out of it.  Ignored while a text field has focus, so typing elsewhere can't delete
/// tiles.
fn delete_tiles(world: World, input: Res<UiInput>, focus: Res<UiFocus>, editor: Res<DeltosEditor>) {
    let pressed = input.keys
        .iter()
        .any(|(key, _)| matches!(key, Key::Named(NamedKey::Delete | NamedKey::Backspace)));
    if !pressed || focus.focused.is_some() { return }

    // find the hovered tile and its graph
    let Some(hovered) = input.hovered else { return };
    let tile = std::iter::once(hovered)
        .chain(world.hierarchy().ancestors(hovered))
        .find_map(|id| component::<TileNode>(&world, id));
    let Some(TileNode { tile_id }) = tile else { return };
    let Some(root) = root_of(&world, hovered) else { return };
    let Some(GraphRoot { graph, .. }) = component::<GraphRoot>(&world, root) else { return };
    if !editable(&world, root) { return }

    // remove it from the graph, then from every view of the graph, and redraw the wires
    let result = graph.remove_tile(tile_id, &editor.theme);
    if let Err(err) = &result {
        eprintln!("Failed to delete tile {tile_id}: {err:#}");
    }
    if result.is_ok() {
        let views: Vec<EntityID> = children_with::<TileNode>(&world)
            .into_iter()
            .filter(|(id, _)| component::<TileNode>(&world, *id).is_some_and(|tile| tile.tile_id == tile_id))
            .filter(|(_, parent)| component::<GraphRoot>(&world, *parent).is_some_and(|other| Arc::ptr_eq(&other.graph, &graph)))
            .map(|(id, _)| id)
            .collect();
        for id in views { world.handle(id).despawn_recursive(); }
    }
    respawn_wires(&world, &editor.theme, &graph);
    mark_dirty(&world, root);
}

/// Applies a property field's text to its tile when the field is submitted, then shows the
/// property's value in the field again, so a refused value goes back to the last good one.
fn edit_properties(world: World, submitted: Event<Submit>, mut editor: ResMut<DeltosEditor>) {
    for Submit { entity, text } in submitted.read() {
        let Some(field) = component::<PropertyField>(&world, entity) else { continue };
        let Some(root) = root_of(&world, entity) else { continue };
        let Some(GraphRoot { graph, .. }) = component::<GraphRoot>(&world, root) else { continue };
        if !editable(&world, root) { continue }

        // set the property, warning when the tile refuses it
        let current = graph
            .properties(field.tile_id)
            .into_iter()
            .find(|property| *property.name() == field.name)
            .map(|property| property.value().clone());
        if current.as_ref() != Some(&text) {
            match graph.set_property(field.tile_id, &field.name, &text) {
                Ok(()) => mark_dirty(&world, root),
                Err(err) => editor.notify(NotifyLevel::Warning, format!("Couldn't set {}: {err:#}", field.name))
            }
        }

        // show the property's value as the tile now has it
        let shown = graph
            .properties(field.tile_id)
            .into_iter()
            .find(|property| *property.name() == field.name);
        if let Some(shown) = shown
            && shown.value() != &text
            && let Ok(Some(mut input)) = world.handle(entity).get::<&mut TextInput>() {
            *input = editor::property_input(shown.value(), input.disabled);
        }
    }
}

/// Writes every dirty, writable graph back to its file in the background when save is pressed.
/// Writes only replace the version of the file that was loaded, so a file changed remotely isn't
/// overwritten.
fn save_graphs(mut editor: ResMut<DeltosEditor>, input: Res<UiInput>, mut graphs: Query<(&'static GraphRoot, &'static OpenFile)>) {
    let command = if cfg!(target_os = "macos") { input.modifiers.super_key() } else { input.modifiers.control_key() };
    let pressed_s = input.keys.iter().any(|(key, _)| matches!(key, Key::Character(c) if c.eq_ignore_ascii_case("s")));
    if !(command && pressed_s) { return }

    while let Ok(Some((entity, (root, file)))) = graphs.next() {
        // skip files that are clean or can't be written, and graphs that can't be saved
        let saved = save_target(&editor, &file).and_then(|source| match source {
            Some(source) => Ok(Some((source, root.graph.save()?))),
            None => Ok(None)
        });
        let (source, text) = match saved {
            Ok(Some(saved)) => saved,
            Ok(None) => continue,
            Err(err) => {
                editor.notify(NotifyLevel::Error, format!("Couldn't save {}: {err:#}", file.path.display()));
                continue;
            }
        };

        // write the graph, then mark the file saved if the graph wasn't edited while writing
        let (graph, path, version) = (root.graph.clone(), file.path.clone(), file.version.clone());
        editor.tasks.spawn(Box::pin(async move {
            let result = source.write_file_text_if(path.clone(), text.clone(), version).await;
            let apply: TaskResult = Box::new(move |world: &World, editor: &mut DeltosEditor| match result {
                Ok(version) => mark_saved(world, entity, &graph, &text, version),
                Err(err) if is_conflict(&err) => editor.notify(NotifyLevel::Warning,
                    format!("{} changed remotely, so it wasn't saved. Reopen it to see the new version.", path.display())),
                Err(err) => editor.notify(NotifyLevel::Error, format!("Couldn't save {}: {err:#}", path.display()))
            });
            apply
        }));
    }
}

/// Records a graph file's new version once `saved` is written, and clears its dirty flag unless
/// the graph changed since.
fn mark_saved(world: &World, entity: EntityID, graph: &Arc<dyn SharedGraph>, saved: &str, version: Option<String>) {
    let unchanged = graph
        .save()
        .is_ok_and(|text| text == saved);
    if let Ok(Some(mut file)) = world.handle(entity).get::<&mut OpenFile>() {
        file.version = version;
        if unchanged { file.dirty = false }
    }
}

/// Whether the graph drawn by `root` can be changed: it wasn't opened read only.
fn editable(world: &World, root: EntityID) -> bool {
    component::<OpenFile>(world, root).is_none_or(|file| !file.read_only)
}

/// Marks the file of the graph drawn by `root` as having unsaved changes.
fn mark_dirty(world: &World, root: EntityID) {
    if let Ok(Some(mut file)) = world.handle(root).get::<&mut OpenFile>()
        && !file.read_only {
        file.dirty = true;
    }
}

/// Replaces the drawn wires of every view of `graph` with the graph's current ones.
fn respawn_wires(world: &World, theme: &Theme, graph: &Arc<dyn SharedGraph>) {
    let wires = graph.wires(theme);
    let roots: Vec<EntityID> = Query::<&GraphRoot>::new(world)
        .iter()
        .filter(|(_, root)| Arc::ptr_eq(&root.graph, graph))
        .map(|(id, _)| id)
        .collect();
    for root in roots {
        let old: Vec<EntityID> = children_with::<WireNode>(world)
            .into_iter()
            .filter(|(_, parent)| *parent == root)
            .map(|(id, _)| id)
            .collect();
        for id in old { world.handle(id).despawn_recursive(); }
        for wire in &wires {
            world.spawn_with_parent(Some(root), editor::wire_bundle(wire.clone()).into_entity_builder());
        }
    }
}

/// Draws the started wire from `pending` to the pointer, or to `hovered` if it can finish the
/// wire, and hides the previews of other graphs.  The preview takes the output's color, or the
/// started input's color until an output is hovered.
fn draw_preview(world: &World, input: &UiInput, pending: Option<EntityID>, hovered: Option<EntityID>) {
    let scale = scale(input);
    let preview = pending.and_then(|start| {
        let dot = component::<IODot>(world, start)?;
        let root = root_of(world, start)?;
        let from = center_in(world, root, start, scale)?;

        // snap to a hovered dot on the other side, otherwise follow the pointer
        let target = hovered
            .and_then(|id| Some((id, component::<IODot>(world, id)?)))
            .filter(|(id, target)| target.output != dot.output && root_of(world, *id) == Some(root))
            .and_then(|(id, target)| Some((center_in(world, root, id, scale)?, target)));
        let to = match &target {
            Some((center, _)) => *center,
            None => (input.cursor - computed_rect(world, root)?.0) / scale
        };

        // wires run from the output to the input, in the output's color
        let color = if dot.output { dot.color } else { target.as_ref().map_or(dot.color, |(_, target)| target.color) };
        let (start, end) = if dot.output { (from, to) } else { (to, from) };
        Some((root, start, end, color))
    });

    for (id, root) in children_with::<WirePreview>(world) {
        let Ok(Some(mut node)) = world.handle(id).get::<&mut Node>() else { continue };
        match preview {
            Some((preview_root, start, end, color)) if preview_root == root => {
                editor::draw_wire(&mut node, start, end, color, editor::WIRE_THICKNESS);
                set_shown(&mut node, true);
            }
            _ => set_shown(&mut node, false)
        }
    }
}

/// Keeps every wire drawn between its output's and input's dots, so wires follow dragged tiles,
/// and draws the wire under the pointer a little wider.  Wires whose dots aren't shown are hidden.
fn draw_wires(world: World, input: Res<UiInput>, layout: Res<EntityUiElements>) {
    let scale = scale(&input);
    let hovered = wire_under_pointer(&world, &input, &layout);

    // find every dot's center in its graph
    let dots: Vec<(EntityID, IODot)> = Query::<&IODot>::new(&world)
        .iter()
        .map(|(id, dot)| (id, dot.clone()))
        .collect();
    let mut centers = HashMap::new();
    for (id, dot) in dots {
        let Some(root) = root_of(&world, id) else { continue };
        if let Some(center) = center_in(&world, root, id, scale) {
            centers.insert((root, dot.tile_id, dot.button_id, dot.output), center);
        }
    }

    // redraw the wires whose ends moved or that were hovered or left
    let wires: Vec<(EntityID, WireNode)> = Query::<&WireNode>::new(&world)
        .iter()
        .map(|(id, wire)| (id, wire.clone()))
        .collect();
    for (id, wire) in wires {
        let Some(root) = world.hierarchy().parent(id) else { continue };
        let start = centers.get(&(root, wire.output_tile, wire.output_button, true));
        let end = centers.get(&(root, wire.input_tile, wire.input_button, false));
        let ends = start.copied().zip(end.copied());
        let is_hovered = hovered == Some(id);
        if ends == wire.drawn && is_hovered == wire.hovered { continue }

        if let Ok(Some(mut node)) = world.handle(id).get::<&mut Node>() {
            let thickness = if is_hovered { editor::WIRE_HOVER_THICKNESS } else { editor::WIRE_THICKNESS };
            if let Some((start, end)) = ends { editor::draw_wire(&mut node, start, end, wire.color, thickness) }
            set_shown(&mut node, ends.is_some());
        }
        if let Ok(Some(mut wire)) = world.handle(id).get::<&mut WireNode>() {
            wire.drawn = ends;
            wire.hovered = is_hovered;
        }
    }
}

/// Whether an input or output with `rules` and `count` wires can take another.
fn allows_another(rules: WireRules, count: usize) -> bool {
    match rules {
        WireRules::Single => count < 1,
        WireRules::Multi => true,
        WireRules::Capped(cap) => count < cap
    }
}

/// Window pixels per logical pixel.
fn scale(input: &UiInput) -> f32 { input.scale.max(f32::EPSILON) }

/// Whether `entity` has a `T`.
fn has<T: ComponentMeta + Sync>(world: &World, entity: EntityID) -> bool {
    world.handle(entity).get::<&T>().ok().flatten().is_some()
}

/// A copy of `entity`'s `T`, if it has one.
fn component<T: ComponentMeta + Clone + Sync>(world: &World, entity: EntityID) -> Option<T> {
    world
        .handle(entity)
        .get::<&T>()
        .ok()
        .flatten()
        .map(|found| found.clone())
}

/// Every node with a `T`, paired with its parent.
fn children_with<T: ComponentMeta + Sync>(world: &World) -> Vec<(EntityID, EntityID)> {
    let ids: Vec<EntityID> = Query::<&T>::new(world)
        .iter()
        .map(|(id, _)| id)
        .collect();
    ids
        .into_iter()
        .filter_map(|id| Some((id, world.hierarchy().parent(id)?)))
        .collect()
}

/// The `GraphRoot` node `entity` is drawn in.
fn root_of(world: &World, entity: EntityID) -> Option<EntityID> {
    world
        .hierarchy()
        .ancestors(entity)
        .find(|ancestor| has::<GraphRoot>(world, *ancestor))
}

/// The laid out position and size of `entity`'s node in window pixels.
fn computed_rect(world: &World, entity: EntityID) -> Option<(Vec2, Vec2)> {
    world
        .handle(entity)
        .get::<&ComputedNode>()
        .ok()
        .flatten()
        .map(|computed| (computed.position, computed.size))
}

/// The center of `entity`'s node in logical pixels from `root`'s top left.
fn center_in(world: &World, root: EntityID, entity: EntityID, scale: f32) -> Option<Vec2> {
    let (root_position, _) = computed_rect(world, root)?;
    let (position, size) = computed_rect(world, entity)?;
    Some((position + size / 2.0 - root_position) / scale)
}

/// Places `node` at `offset` logical pixels from its parent's top left.
pub(crate) fn set_top_left(node: &mut Node, offset: Vec2) {
    let edges = Rect::new_top_left(px(offset.y), px(offset.x));

    // only touch the style when the offset actually changes
    let unchanged = matches!(node.style().position_type(), PositionType::Absolute(current) if *current == edges);
    if !unchanged {
        node.style_mut().set_position_type(PositionType::Absolute(edges));
    }
}

/// Shows or hides `node`, only touching its style when that changes.
fn set_shown(node: &mut Node, shown: bool) {
    let display = if shown { Display::Flex } else { Display::None };
    if *node.style().display() != display {
        node.style_mut().set_display(display);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_rules_cap_wire_counts() {
        assert!(allows_another(WireRules::Single, 0));
        assert!(!allows_another(WireRules::Single, 1));
        assert!(allows_another(WireRules::Multi, 1_000));
        assert!(allows_another(WireRules::Capped(2), 1));
        assert!(!allows_another(WireRules::Capped(2), 2));
        assert!(!allows_another(WireRules::Capped(0), 0));
    }

    use crate::tiles::{IOButton, Wire, Wires};

    /// ID of every test tile's only input.
    const INPUT: Uuid = Uuid::from_u128(1);

    /// ID of every test tile's only output.
    const OUTPUT: Uuid = Uuid::from_u128(2);

    /// A tile with one input, one output, and a whole number `size` property.
    ///
    /// Fields:
    /// * tile_id: Unique ID of this tile.
    /// * wires: The wires out of its output.
    /// * size: The tile's only property.
    #[derive(Debug)]
    struct TestTile {
        tile_id: Uuid,
        wires: Vec<Wire>,
        size: u32
    }

    impl NodeGraphTile for TestTile {
        fn tile_id(&self) -> Uuid { self.tile_id }
        fn title(&self) -> String { "Test".into() }
        fn color(&self, theme: &Theme) -> Vec4 { theme.orange }
        fn position(&self) -> Vec2 { Vec2::ZERO }
        fn set_position(&mut self, _position: Vec2) {}

        fn connect(&mut self, _output_button_id: Uuid, input_tile_id: Uuid, input_button_id: Uuid) -> anyhow::Result<()> {
            self.wires.push(Wire::new(input_tile_id, input_button_id));
            Ok(())
        }

        fn disconnect(&mut self, _output_button_id: Uuid, input_tile_id: Uuid, input_button_id: Uuid) -> anyhow::Result<()> {
            let wire = Wire::new(input_tile_id, input_button_id);
            self.wires.retain(|existing| *existing != wire);
            Ok(())
        }

        fn inputs(&self, theme: &Theme) -> Vec<IOButton> {
            vec![IOButton::new(INPUT, "In", theme.blue, WireRules::Multi)]
        }

        fn outputs(&self, theme: &Theme) -> Vec<(IOButton, Wires)> {
            vec![(IOButton::new(OUTPUT, "Out", theme.purple, WireRules::Multi), Wires::new(self.wires.clone()))]
        }

        fn properties(&self) -> Vec<TileProperty> { vec![TileProperty::new("size", self.size.to_string())] }

        fn set_property(&mut self, name: &str, value: &str) -> anyhow::Result<()> {
            anyhow::ensure!(name == "size", "No property {name}");
            self.size = value.parse()?;
            Ok(())
        }
    }

    /// A graph of `TestTile`s that saves as its tile count.
    ///
    /// Fields:
    /// * tiles: The tiles of the graph.
    #[derive(Debug)]
    struct TestGraph {
        tiles: Vec<TestTile>
    }

    impl NodeGraphEditor for TestGraph {
        type Tile = TestTile;

        fn name(&self) -> String { "Test".into() }
        fn file_types(&self) -> Vec<String> { vec!["test".into()] }

        fn open(content: &str) -> anyhow::Result<Self> {
            let count: usize = content.parse()?;
            let tiles = (0..count)
                .map(|_| TestTile { tile_id: Uuid::new_v4(), wires: Vec::new(), size: 1 })
                .collect();
            Ok(Self { tiles })
        }

        fn save(&self) -> anyhow::Result<String> { Ok(self.tiles.len().to_string()) }
        fn new_tile_options(&self) -> Vec<String> { Vec::new() }
        fn new_tile(&mut self, _name: String, _position: Vec2) {}

        fn remove_tile(&mut self, tile_id: Uuid) -> anyhow::Result<()> {
            self.tiles.retain(|tile| tile.tile_id != tile_id);
            Ok(())
        }

        fn tiles(&self) -> &[TestTile] { &self.tiles }
        fn tiles_mut(&mut self) -> &mut [TestTile] { &mut self.tiles }
    }

    /// Opens a graph of three tiles where the first and last both feed the middle one, which
    /// feeds the last, and gives the tile IDs in order.
    fn wired_graph() -> anyhow::Result<(Mutex<TestGraph>, [Uuid; 3])> {
        let graph = Mutex::new(TestGraph::open("3")?);
        let ids: [Uuid; 3] = lock(&graph)
            .tiles()
            .iter()
            .map(|tile| tile.tile_id())
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| anyhow::anyhow!("expected three tiles"))?;

        // wire first -> middle, last -> middle, middle -> last
        let [first, middle, last] = ids;
        graph.connect(first, OUTPUT, middle, INPUT)?;
        graph.connect(last, OUTPUT, middle, INPUT)?;
        graph.connect(middle, OUTPUT, last, INPUT)?;
        Ok((graph, ids))
    }

    #[test]
    fn removing_a_tile_drops_every_wire_into_and_out_of_it() -> anyhow::Result<()> {
        let theme = Theme::default();
        let (graph, [first, middle, last]) = wired_graph()?;

        graph.remove_tile(middle, &theme)?;

        let tiles = lock(&graph)
            .tiles()
            .iter()
            .map(|tile| tile.tile_id())
            .collect::<Vec<_>>();
        assert_eq!(tiles, [first, last]);
        assert!(graph.wires(&theme).is_empty(), "wires left: {:?}", graph.wires(&theme));
        Ok(())
    }

    #[test]
    fn removing_a_tile_keeps_unrelated_wires() -> anyhow::Result<()> {
        let theme = Theme::default();
        let (graph, [first, middle, last]) = wired_graph()?;

        graph.remove_tile(last, &theme)?;

        let wires = graph
            .wires(&theme)
            .into_iter()
            .map(|wire| (wire.output_tile, wire.input_tile))
            .collect::<Vec<_>>();
        assert_eq!(wires, [(first, middle)]);
        Ok(())
    }

    #[test]
    fn removing_a_missing_tile_fails_without_changes() -> anyhow::Result<()> {
        let theme = Theme::default();
        let (graph, _) = wired_graph()?;

        assert!(graph.remove_tile(Uuid::new_v4(), &theme).is_err());

        assert_eq!(lock(&graph).tiles().len(), 3);
        assert_eq!(graph.wires(&theme).len(), 3);
        Ok(())
    }

    #[test]
    fn properties_are_set_on_the_right_tile() -> anyhow::Result<()> {
        let (graph, [first, middle, _]) = wired_graph()?;

        graph.set_property(middle, "size", "7")?;

        assert_eq!(graph.properties(middle), [TileProperty::new("size", "7")]);
        assert_eq!(graph.properties(first), [TileProperty::new("size", "1")]);
        Ok(())
    }

    #[test]
    fn refused_properties_leave_tiles_unchanged() -> anyhow::Result<()> {
        let (graph, [first, _, _]) = wired_graph()?;

        assert!(graph.set_property(first, "size", "big").is_err());
        assert!(graph.set_property(first, "missing", "2").is_err());
        assert!(graph.set_property(Uuid::new_v4(), "size", "2").is_err());

        assert_eq!(graph.properties(first), [TileProperty::new("size", "1")]);
        assert!(graph.properties(Uuid::new_v4()).is_empty());
        Ok(())
    }

    #[test]
    fn shared_graphs_save_through_their_editor() -> anyhow::Result<()> {
        let graph = Mutex::new(TestGraph::open("2")?);

        assert_eq!(SharedGraph::save(&graph)?, "2");
        Ok(())
    }
}
