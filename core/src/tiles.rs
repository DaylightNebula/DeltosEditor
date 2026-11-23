use gearbox::glam::Vec2;
use uuid::Uuid;

/// Fraction of a tile's size, measured in from each edge, that drops a tab beside the tile
/// instead of into it.
pub const EDGE_DROP_FRACTION: f32 = 0.25;

/// Which way a split lays out its two halves.
///
/// Variants:
/// * Horizontal: The halves sit side by side, `first` on the left.
/// * Vertical: The halves are stacked, `first` on top.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SplitAxis {
    Horizontal,
    Vertical
}

/// Where a dragged tab is dropped relative to a tile.
///
/// Variants:
/// * Center: Into the tile, joining its tabs.
/// * Left: Into a new tile left of the tile.
/// * Right: Into a new tile right of the tile.
/// * Top: Into a new tile above the tile.
/// * Bottom: Into a new tile below the tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DropZone {
    Center,
    Left,
    Right,
    Top,
    Bottom
}

impl DropZone {
    /// Finds the zone of a tile's content area that a point falls in.
    ///
    /// A point within `EDGE_DROP_FRACTION` of an edge picks the closest edge, any other point
    /// picks `Center`.  Points outside the area are clamped onto it.
    ///
    /// # Parameters
    /// * position: Top left corner of the area.
    /// * size: Size of the area.
    /// * point: The point to find the zone of, in the same space as `position`.
    pub fn at(position: Vec2, size: Vec2, point: Vec2) -> Self {
        // normalize the point into the area, so the edge band scales with the tile
        let local = ((point - position) / size.max(Vec2::splat(f32::EPSILON))).clamp(Vec2::ZERO, Vec2::ONE);
        let edges = [
            (local.x, DropZone::Left),
            (1.0 - local.x, DropZone::Right),
            (local.y, DropZone::Top),
            (1.0 - local.y, DropZone::Bottom),
        ];

        // pick the closest edge, if the point is near enough to it
        edges.into_iter()
            .filter(|(distance, _)| *distance < EDGE_DROP_FRACTION)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, zone)| zone)
            .unwrap_or(DropZone::Center)
    }

    /// The part of an area a tab dropped in this zone will take, as a position and size.
    ///
    /// Edge zones take the half of the area on their side, and `Center` takes all of it.
    ///
    /// # Parameters
    /// * position: Top left corner of the area.
    /// * size: Size of the area.
    pub fn preview(self, position: Vec2, size: Vec2) -> (Vec2, Vec2) {
        let half = size * 0.5;
        match self {
            DropZone::Center => (position, size),
            DropZone::Left => (position, Vec2::new(half.x, size.y)),
            DropZone::Right => (position + Vec2::new(half.x, 0.0), Vec2::new(half.x, size.y)),
            DropZone::Top => (position, Vec2::new(size.x, half.y)),
            DropZone::Bottom => (position + Vec2::new(0.0, half.y), Vec2::new(size.x, half.y)),
        }
    }

    /// The axis and side of a split made by dropping in this zone, `None` for `Center`.  The
    /// side is `true` when the new tile goes first (left or top).
    fn split(self) -> Option<(SplitAxis, bool)> {
        match self {
            DropZone::Center => None,
            DropZone::Left => Some((SplitAxis::Horizontal, true)),
            DropZone::Right => Some((SplitAxis::Horizontal, false)),
            DropZone::Top => Some((SplitAxis::Vertical, true)),
            DropZone::Bottom => Some((SplitAxis::Vertical, false)),
        }
    }
}

/// How the editor's tiles are arranged, as a tree of splits with tiles at the leaves.
///
/// Variants:
/// * Tile: A single tile, by its `EditorTile` ID.
/// * Split: Two layouts sharing the space along `axis`, `first` being the left or top one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TileLayout {
    Tile(Uuid),
    Split { axis: SplitAxis, first: Box<TileLayout>, second: Box<TileLayout> }
}

impl TileLayout {
    /// Lists every tile in the layout, left to right and top to bottom.
    pub fn tiles(&self) -> Vec<Uuid> {
        let mut tiles = Vec::new();
        self.collect_tiles(&mut tiles);
        tiles
    }

    /// Pushes every tile in the layout onto `tiles` in order.
    fn collect_tiles(&self, tiles: &mut Vec<Uuid>) {
        match self {
            TileLayout::Tile(id) => tiles.push(*id),
            TileLayout::Split { first, second, .. } => {
                first.collect_tiles(tiles);
                second.collect_tiles(tiles);
            }
        }
    }

    /// Whether `tile` is somewhere in the layout.
    ///
    /// # Parameters
    /// * tile: ID of the tile to look for.
    pub fn contains(&self, tile: Uuid) -> bool {
        match self {
            TileLayout::Tile(id) => *id == tile,
            TileLayout::Split { first, second, .. } => first.contains(tile) || second.contains(tile),
        }
    }

    /// Splits the `target` tile in two, putting `new_tile` on the side `zone` names.
    ///
    /// Returns whether the split was made.  Nothing changes if `target` isn't in the layout,
    /// `new_tile` already is, or `zone` is `Center`.
    ///
    /// # Parameters
    /// * target: ID of the tile to split.
    /// * new_tile: ID of the tile to add beside `target`.
    /// * zone: Which side of `target` the new tile goes on.
    pub fn split(&mut self, target: Uuid, new_tile: Uuid, zone: DropZone) -> bool {
        let Some((axis, new_first)) = zone.split() else { return false };
        if self.contains(new_tile) { return false }
        self.split_inner(target, new_tile, axis, new_first)
    }

    /// Replaces the leaf of `target` with a split of it and `new_tile`.
    fn split_inner(&mut self, target: Uuid, new_tile: Uuid, axis: SplitAxis, new_first: bool) -> bool {
        match self {
            TileLayout::Tile(id) if *id == target => {
                let (old, new) = (Box::new(TileLayout::Tile(target)), Box::new(TileLayout::Tile(new_tile)));
                let (first, second) = if new_first { (new, old) } else { (old, new) };
                *self = TileLayout::Split { axis, first, second };
                true
            }
            TileLayout::Tile(_) => false,
            TileLayout::Split { first, second, .. } =>
                first.split_inner(target, new_tile, axis, new_first) || second.split_inner(target, new_tile, axis, new_first),
        }
    }

    /// Removes `tile` from the layout, giving its space to the other half of its split.
    ///
    /// Returns whether the tile was removed.  The last tile can't be removed, as the layout
    /// always holds at least one.
    ///
    /// # Parameters
    /// * tile: ID of the tile to remove.
    pub fn remove(&mut self, tile: Uuid) -> bool {
        match self {
            TileLayout::Tile(_) => false,
            TileLayout::Split { first, second, .. } => {
                // collapse this split into the other half when one half is the tile
                let keep = match (first.as_ref(), second.as_ref()) {
                    (TileLayout::Tile(id), _) if *id == tile => Some(second.as_ref().clone()),
                    (_, TileLayout::Tile(id)) if *id == tile => Some(first.as_ref().clone()),
                    _ => None,
                };
                match keep {
                    Some(keep) => {
                        *self = keep;
                        true
                    }
                    None => first.remove(tile) || second.remove(tile),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds `count` fresh tile IDs.
    fn ids(count: usize) -> Vec<Uuid> {
        (0..count)
            .map(|_| Uuid::new_v4())
            .collect()
    }

    #[test]
    fn split_places_new_tile_on_each_side() {
        let [a, b] = ids(2)[..] else { unreachable!() };
        let cases = [
            (DropZone::Left, SplitAxis::Horizontal, vec![b, a]),
            (DropZone::Right, SplitAxis::Horizontal, vec![a, b]),
            (DropZone::Top, SplitAxis::Vertical, vec![b, a]),
            (DropZone::Bottom, SplitAxis::Vertical, vec![a, b]),
        ];
        for (zone, axis, order) in cases {
            let mut layout = TileLayout::Tile(a);
            assert!(layout.split(a, b, zone), "{zone:?} didn't split");
            assert!(matches!(layout, TileLayout::Split { axis: got, .. } if got == axis), "{zone:?} used the wrong axis");
            assert_eq!(layout.tiles(), order, "{zone:?} put the tiles in the wrong order");
        }
    }

    #[test]
    fn split_refuses_center_missing_target_and_duplicates() {
        let [a, b, c] = ids(3)[..] else { unreachable!() };
        let mut layout = TileLayout::Tile(a);
        assert!(!layout.split(a, b, DropZone::Center));
        assert!(!layout.split(c, b, DropZone::Left));
        assert!(!layout.split(a, a, DropZone::Left));
        assert_eq!(layout, TileLayout::Tile(a));
    }

    #[test]
    fn split_nests_inside_existing_splits() {
        let [a, b, c] = ids(3)[..] else { unreachable!() };
        let mut layout = TileLayout::Tile(a);
        layout.split(a, b, DropZone::Right);
        layout.split(b, c, DropZone::Bottom);

        assert_eq!(layout.tiles(), vec![a, b, c]);
        let TileLayout::Split { axis: SplitAxis::Horizontal, second, .. } = &layout else { panic!("outer split lost: {layout:?}") };
        assert!(matches!(second.as_ref(), TileLayout::Split { axis: SplitAxis::Vertical, .. }));
    }

    #[test]
    fn remove_collapses_split_into_sibling() {
        let [a, b, c] = ids(3)[..] else { unreachable!() };
        let mut layout = TileLayout::Tile(a);
        layout.split(a, b, DropZone::Right);
        layout.split(b, c, DropZone::Bottom);

        assert!(layout.remove(b));
        assert_eq!(layout.tiles(), vec![a, c]);
        assert!(layout.remove(a));
        assert_eq!(layout, TileLayout::Tile(c));
    }

    #[test]
    fn remove_keeps_last_tile_and_ignores_unknown() {
        let [a, b] = ids(2)[..] else { unreachable!() };
        let mut layout = TileLayout::Tile(a);
        assert!(!layout.remove(a));
        assert!(!layout.remove(b));
        assert_eq!(layout, TileLayout::Tile(a));
    }

    #[test]
    fn drop_zone_picks_nearest_edge_or_center() {
        let (position, size) = (Vec2::new(100.0, 50.0), Vec2::new(200.0, 100.0));
        let at = |x: f32, y: f32| DropZone::at(position, size, position + size * Vec2::new(x, y));

        assert_eq!(at(0.5, 0.5), DropZone::Center);
        assert_eq!(at(0.05, 0.5), DropZone::Left);
        assert_eq!(at(0.95, 0.5), DropZone::Right);
        assert_eq!(at(0.5, 0.05), DropZone::Top);
        assert_eq!(at(0.5, 0.95), DropZone::Bottom);

        // corners go to whichever edge is closer
        assert_eq!(at(0.02, 0.1), DropZone::Left);
        assert_eq!(at(0.1, 0.02), DropZone::Top);

        // points outside the area clamp onto it
        assert_eq!(at(-1.0, 0.5), DropZone::Left);
    }

    #[test]
    fn drop_zone_preview_covers_its_half() {
        let (position, size) = (Vec2::new(10.0, 20.0), Vec2::new(100.0, 60.0));
        assert_eq!(DropZone::Center.preview(position, size), (position, size));
        assert_eq!(DropZone::Left.preview(position, size), (position, Vec2::new(50.0, 60.0)));
        assert_eq!(DropZone::Right.preview(position, size), (Vec2::new(60.0, 20.0), Vec2::new(50.0, 60.0)));
        assert_eq!(DropZone::Top.preview(position, size), (position, Vec2::new(100.0, 30.0)));
        assert_eq!(DropZone::Bottom.preview(position, size), (Vec2::new(10.0, 50.0), Vec2::new(100.0, 30.0)));
    }
}
