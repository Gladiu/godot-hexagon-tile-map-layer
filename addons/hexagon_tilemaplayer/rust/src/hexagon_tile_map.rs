//! Rust conversion of `hexagon_tilemap.gd`.
//!
//! [`HexagonTileMap`] is a stateless utility class. All cube-coordinate math and
//! layout conversions live here, so they can be used both from GDScript (as
//! static class methods) and from [`HexagonTileMapLayer`](super::hexagon_tile_map_layer::HexagonTileMapLayer).

use std::collections::HashMap;

use godot::builtin::{real, real_consts::PI};
use godot::classes::tile_set::{CellNeighbor, TileLayout, TileOffsetAxis};
use godot::classes::{CapsuleShape2D, Shape2D};
use godot::prelude::*;

use crate::hexagon_tile_map_layer::HexagonTileMapLayer;

pub(crate) const WEDGE_COUNT: i64 = 6;

pub(crate) fn wedge_angle() -> real {
    PI / 3.0
}

pub(crate) fn wedge_inv_angle() -> real {
    1.0 / wedge_angle()
}

pub(crate) fn wedge_angle_half() -> real {
    PI / 6.0
}

#[allow(dead_code)]
pub(crate) fn wedge_inv_angle_half() -> real {
    1.0 / wedge_angle_half()
}

// ---------------------------------------------------------------------------
// Direction tables
// ---------------------------------------------------------------------------

pub(crate) fn side_directions(axis: TileOffsetAxis) -> Vec<CellNeighbor> {
    if axis == TileOffsetAxis::HORIZONTAL {
        vec![
            CellNeighbor::TOP_RIGHT_SIDE,
            CellNeighbor::RIGHT_SIDE,
            CellNeighbor::BOTTOM_RIGHT_SIDE,
            CellNeighbor::BOTTOM_LEFT_SIDE,
            CellNeighbor::LEFT_SIDE,
            CellNeighbor::TOP_LEFT_SIDE,
        ]
    } else {
        vec![
            CellNeighbor::TOP_RIGHT_SIDE,
            CellNeighbor::BOTTOM_RIGHT_SIDE,
            CellNeighbor::BOTTOM_SIDE,
            CellNeighbor::BOTTOM_LEFT_SIDE,
            CellNeighbor::TOP_LEFT_SIDE,
            CellNeighbor::TOP_SIDE,
        ]
    }
}

pub(crate) fn corner_directions(axis: TileOffsetAxis) -> Vec<CellNeighbor> {
    if axis == TileOffsetAxis::HORIZONTAL {
        vec![
            CellNeighbor::TOP_RIGHT_CORNER,
            CellNeighbor::BOTTOM_RIGHT_CORNER,
            CellNeighbor::BOTTOM_CORNER,
            CellNeighbor::BOTTOM_LEFT_CORNER,
            CellNeighbor::TOP_LEFT_CORNER,
            CellNeighbor::TOP_CORNER,
        ]
    } else {
        vec![
            CellNeighbor::TOP_RIGHT_CORNER,
            CellNeighbor::RIGHT_CORNER,
            CellNeighbor::BOTTOM_RIGHT_CORNER,
            CellNeighbor::BOTTOM_LEFT_CORNER,
            CellNeighbor::LEFT_CORNER,
            CellNeighbor::TOP_LEFT_CORNER,
        ]
    }
}

pub(crate) fn direction_vector(axis: TileOffsetAxis, direction: CellNeighbor) -> Vector3i {
    if axis == TileOffsetAxis::HORIZONTAL {
        match direction {
            CellNeighbor::TOP_RIGHT_SIDE => Vector3i::new(1, -1, 0),
            CellNeighbor::RIGHT_SIDE => Vector3i::new(1, 0, -1),
            CellNeighbor::BOTTOM_RIGHT_SIDE => Vector3i::new(0, 1, -1),
            CellNeighbor::BOTTOM_LEFT_SIDE => Vector3i::new(-1, 1, 0),
            CellNeighbor::LEFT_SIDE => Vector3i::new(-1, 0, 1),
            CellNeighbor::TOP_LEFT_SIDE => Vector3i::new(0, -1, 1),
            CellNeighbor::TOP_RIGHT_CORNER => Vector3i::new(2, -1, -1),
            CellNeighbor::BOTTOM_RIGHT_CORNER => Vector3i::new(1, 1, -2),
            CellNeighbor::BOTTOM_CORNER => Vector3i::new(-1, 2, -1),
            CellNeighbor::BOTTOM_LEFT_CORNER => Vector3i::new(-2, 1, 1),
            CellNeighbor::TOP_LEFT_CORNER => Vector3i::new(-1, -1, 2),
            CellNeighbor::TOP_CORNER => Vector3i::new(1, -2, 1),
            _ => Vector3i::ZERO,
        }
    } else {
        match direction {
            CellNeighbor::TOP_RIGHT_SIDE => Vector3i::new(1, -1, 0),
            CellNeighbor::BOTTOM_RIGHT_SIDE => Vector3i::new(1, 0, -1),
            CellNeighbor::BOTTOM_SIDE => Vector3i::new(0, 1, -1),
            CellNeighbor::BOTTOM_LEFT_SIDE => Vector3i::new(-1, 1, 0),
            CellNeighbor::TOP_LEFT_SIDE => Vector3i::new(-1, 0, 1),
            CellNeighbor::TOP_SIDE => Vector3i::new(0, -1, 1),
            CellNeighbor::TOP_RIGHT_CORNER => Vector3i::new(1, -2, 1),
            CellNeighbor::RIGHT_CORNER => Vector3i::new(2, -1, -1),
            CellNeighbor::BOTTOM_RIGHT_CORNER => Vector3i::new(1, 1, -2),
            CellNeighbor::BOTTOM_LEFT_CORNER => Vector3i::new(-1, 2, -1),
            CellNeighbor::LEFT_CORNER => Vector3i::new(-2, 1, 1),
            CellNeighbor::TOP_LEFT_CORNER => Vector3i::new(-1, -1, 2),
            _ => Vector3i::ZERO,
        }
    }
}

pub(crate) fn direction_vectors(axis: TileOffsetAxis) -> HashMap<CellNeighbor, Vector3i> {
    let mut map = HashMap::new();
    for direction in side_directions(axis).into_iter().chain(corner_directions(axis)) {
        map.insert(direction, direction_vector(axis, direction));
    }
    map
}

// ---------------------------------------------------------------------------
// Map <-> cube conversion
// ---------------------------------------------------------------------------

pub(crate) fn cube_to_map_for(axis: TileOffsetAxis, layout: TileLayout, cube: Vector3i) -> Vector2i {
    if axis == TileOffsetAxis::HORIZONTAL {
        match layout {
            TileLayout::STACKED => {
                let x = cube.x + ((cube.y & !1) >> 1);
                Vector2i::new(x, cube.y)
            }
            TileLayout::STACKED_OFFSET => {
                let x = cube.x + ((cube.y + 1) >> 1);
                Vector2i::new(x, cube.y)
            }
            TileLayout::STAIRS_RIGHT => Vector2i::new(cube.x, cube.y),
            TileLayout::STAIRS_DOWN => {
                let y = -cube.x;
                let x = -cube.z - y;
                Vector2i::new(x, y)
            }
            TileLayout::DIAMOND_RIGHT => Vector2i::new(cube.x, -cube.z),
            TileLayout::DIAMOND_DOWN => Vector2i::new(-cube.z, -cube.x),
            _ => Vector2i::ZERO,
        }
    } else {
        match layout {
            TileLayout::STACKED => {
                let y = cube.y + ((cube.x - (cube.x & 1)) >> 1);
                Vector2i::new(cube.x, y)
            }
            TileLayout::STACKED_OFFSET => {
                let y = cube.y + ((cube.x + (cube.x & 1)) >> 1);
                Vector2i::new(cube.x, y)
            }
            TileLayout::STAIRS_RIGHT => {
                let x = -cube.y;
                let y = cube.x + 2 * cube.y;
                Vector2i::new(x, y)
            }
            TileLayout::STAIRS_DOWN => Vector2i::new(cube.x, cube.y),
            TileLayout::DIAMOND_RIGHT => Vector2i::new(-cube.y, -cube.z),
            TileLayout::DIAMOND_DOWN => Vector2i::new(-cube.z, cube.y),
            _ => Vector2i::ZERO,
        }
    }
}

pub(crate) fn map_to_cube_for(axis: TileOffsetAxis, layout: TileLayout, map: Vector2i) -> Vector3i {
    if axis == TileOffsetAxis::HORIZONTAL {
        match layout {
            TileLayout::STACKED => {
                let x = map.x - ((map.y & !1) >> 1);
                let y = map.y;
                Vector3i::new(x, y, -x - y)
            }
            TileLayout::STACKED_OFFSET => {
                let x = map.x - ((map.y + 1) >> 1);
                let y = map.y;
                Vector3i::new(x, y, -x - y)
            }
            TileLayout::STAIRS_RIGHT => {
                let x = map.x;
                let y = map.y;
                Vector3i::new(x, y, -x - y)
            }
            TileLayout::STAIRS_DOWN => {
                let x = -map.y;
                let y = map.x + 2 * map.y;
                let z = -map.x - map.y;
                Vector3i::new(x, y, z)
            }
            TileLayout::DIAMOND_RIGHT => {
                let x = map.x;
                let y = map.y - map.x;
                let z = -map.y;
                Vector3i::new(x, y, z)
            }
            TileLayout::DIAMOND_DOWN => {
                let x = -map.y;
                let y = map.x + map.y;
                let z = -map.x;
                Vector3i::new(x, y, z)
            }
            _ => Vector3i::ZERO,
        }
    } else {
        match layout {
            TileLayout::STACKED => {
                let x = map.x;
                let y = map.y - ((map.x - (map.x & 1)) >> 1);
                Vector3i::new(x, y, -x - y)
            }
            TileLayout::STACKED_OFFSET => {
                let x = map.x;
                let y = map.y - ((map.x + (map.x & 1)) >> 1);
                Vector3i::new(x, y, -x - y)
            }
            TileLayout::STAIRS_RIGHT => {
                let x = map.y + 2 * map.x;
                let y = -map.x;
                Vector3i::new(x, y, -x - y)
            }
            TileLayout::STAIRS_DOWN => {
                let x = map.x;
                let y = map.y;
                Vector3i::new(x, y, -x - y)
            }
            TileLayout::DIAMOND_RIGHT => {
                let y = -map.x;
                let z = -map.y;
                Vector3i::new(-y - z, y, z)
            }
            TileLayout::DIAMOND_DOWN => {
                let y = map.y;
                let z = -map.x;
                Vector3i::new(-y - z, y, z)
            }
            _ => Vector3i::ZERO,
        }
    }
}

// ---------------------------------------------------------------------------
// Cube coordinate math
// ---------------------------------------------------------------------------

pub(crate) fn cube_distance(a: Vector3i, b: Vector3i) -> i64 {
    (a.x - b.x).abs().max((a.y - b.y).abs()).max((a.z - b.z).abs()) as i64
}

pub(crate) fn cube_round(frac: Vector3) -> Vector3i {
    let mut x = frac.x.round();
    let mut y = frac.y.round();
    let mut z = frac.z.round();

    let x_diff = (x - frac.x).abs();
    let y_diff = (y - frac.y).abs();
    let z_diff = (z - frac.z).abs();

    if x_diff > y_diff && x_diff > z_diff {
        x = -y - z;
    } else if y_diff > z_diff {
        y = -x - z;
    } else {
        z = -x - y;
    }

    Vector3i::new(x as i32, y as i32, z as i32)
}

pub(crate) fn cube_linedraw(a: Vector3i, b: Vector3i) -> Array<Vector3i> {
    let mut results = Array::<Vector3i>::new();
    if a == b {
        results.push(a);
        return results;
    }

    let distance = cube_distance(a, b);
    let ac = Vector3::new(a.x as real, a.y as real, a.z as real);
    let bc = Vector3::new(b.x as real, b.y as real, b.z as real);
    for index in 0..distance {
        let t = 1.0 / distance as real * index as real;
        results.push(cube_round(ac.lerp(bc, t)));
    }

    if results.at(0) != a {
        results.push_front(a);
    }
    if results.at(results.len() - 1) != b {
        results.push(b);
    }
    results
}

pub(crate) fn cube_range(center: Vector3i, distance: i64) -> Array<Vector3i> {
    let mut results = Array::<Vector3i>::new();
    if distance == 0 {
        results.push(center);
        return results;
    }

    for q in -distance..=distance {
        let r_min = (-distance).max(-q - distance);
        let r_max = distance.min(-q + distance);
        for r in r_min..=r_max {
            let s = -q - r;
            results.push(center + Vector3i::new(q as i32, r as i32, s as i32));
        }
    }
    results
}

pub(crate) fn cube_intersect_ranges(
    center1: Vector3i,
    range1: i64,
    center2: Vector3i,
    range2: i64,
) -> Array<Vector3i> {
    let mut results = Array::<Vector3i>::new();

    let xmin = (center1.x as i64 - range1).max(center2.x as i64 - range2);
    let xmax = (center1.x as i64 + range1).min(center2.x as i64 + range2);
    let ymin = (center1.y as i64 - range1).max(center2.y as i64 - range2);
    let ymax = (center1.y as i64 + range1).min(center2.y as i64 + range2);
    let zmin = (center1.z as i64 - range1).max(center2.z as i64 - range2);
    let zmax = (center1.z as i64 + range1).min(center2.z as i64 + range2);

    for q in xmin..=xmax {
        let r_min = ymin.max(-q - zmax);
        let r_max = ymax.min(-q - zmin);
        for r in r_min..=r_max {
            results.push(Vector3i::new(q as i32, r as i32, (-q - r) as i32));
        }
    }
    results
}

pub(crate) fn cube_rotate(position: Vector3i, rotations: i64) -> Vector3i {
    let mut position = position;
    let mut rotations = rotations;
    if rotations.abs() > WEDGE_COUNT {
        rotations %= WEDGE_COUNT;
    }
    if rotations == 0 {
        return position;
    } else if rotations < 0 {
        for _ in 0..-rotations {
            position = Vector3i::new(-position.z, -position.x, -position.y);
        }
    } else {
        for _ in 0..rotations {
            position = Vector3i::new(-position.y, -position.z, -position.x);
        }
    }
    position
}

pub(crate) fn cube_rotate_from(position: Vector3i, from: Vector3i, rotations: i64) -> Vector3i {
    from + cube_rotate(position - from, rotations)
}

pub(crate) fn cube_reflect(position: Vector3i, axis: i64) -> Vector3i {
    match axis {
        0 => Vector3i::new(position.x, position.z, position.y),
        1 => Vector3i::new(position.z, position.y, position.x),
        2 => Vector3i::new(position.y, position.x, position.z),
        _ => position,
    }
}

pub(crate) fn cube_reflect_from(position: Vector3i, from: Vector3i, axis: i64) -> Vector3i {
    from + cube_reflect(position - from, axis)
}

pub(crate) fn cube_rect_corners(center: Vector3i, corner: Vector3i, axis: i64) -> Array<Vector3i> {
    let mut result = Array::<Vector3i>::new();
    result.resize(4, Vector3i::ZERO);
    result.set(0, corner);
    result.set(1, cube_reflect_from(corner, center, axis));
    let center_twice = center + center;
    result.set(2, center_twice - result.get(0).unwrap());
    result.set(3, center_twice - result.get(1).unwrap());
    result
}

pub(crate) fn cube_rect(center: Vector3i, corner: Vector3i, axis: i64) -> Array<Vector3i> {
    let corners = cube_rect_corners(center, corner, axis);
    let c0 = corners.get(0).unwrap();
    let c1 = corners.get(1).unwrap();
    let c2 = corners.get(2).unwrap();
    let c3 = corners.get(3).unwrap();

    let min_x = c0.x.min(c1.x).min(c2.x).min(c3.x);
    let max_x = c0.x.max(c1.x).max(c2.x).max(c3.x);
    let min_y = c0.y.min(c1.y).min(c2.y).min(c3.y);
    let max_y = c0.y.max(c1.y).max(c2.y).max(c3.y);
    let min_z = c0.z.min(c1.z).min(c2.z).min(c3.z);
    let max_z = c0.z.max(c1.z).max(c2.z).max(c3.z);

    let diag1 = c1 - c0;
    let diag2 = c3 - c0;

    let mut result = Array::<Vector3i>::new();
    for q in min_x..=max_x {
        let r_min = min_y.max(-q - max_z);
        let r_max = max_y.min(-q - min_z);
        for r in r_min..=r_max {
            let s = -q - r;
            if s >= min_z && s <= max_z {
                let point = Vector3i::new(q, r, s);
                let v = point - c0;
                let denom1 = diag1.x as f64 * diag2.y as f64 - diag1.y as f64 * diag2.x as f64;
                let denom2 = diag2.x as f64 * diag1.y as f64 - diag2.y as f64 * diag1.x as f64;
                let a = (v.x as f64 * diag2.y as f64 - v.y as f64 * diag2.x as f64) / denom1;
                let b = (v.x as f64 * diag1.y as f64 - v.y as f64 * diag1.x as f64) / denom2;
                if a >= 0.0 && a <= 1.0 && b >= 0.0 && b <= 1.0 {
                    result.push(point);
                }
            }
        }
    }
    result
}

pub(crate) fn cube_ring_for_neighbor(
    axis: TileOffsetAxis,
    neighbor_directions: &[CellNeighbor],
    center: Vector3i,
    radius: i64,
    first_side: i64,
) -> Array<Vector3i> {
    let mut result = Array::<Vector3i>::new();

    let first_index: i64;
    let mut first_side = first_side;
    if first_side == -1 {
        first_index = 0;
        first_side = neighbor_directions[0].ord() as i64;
    } else {
        first_index = neighbor_directions
            .iter()
            .position(|d| d.ord() as i64 == first_side)
            .map(|i| i as i64)
            .unwrap_or(-1);
    }

    if first_index == -1 {
        godot_error!("Invalid first_side value provided '{}'", first_index);
        return result;
    }
    if radius < 1 {
        result.push(center);
        return result;
    }

    let first_direction = CellNeighbor::try_from_ord(first_side as i32).unwrap();
    let mut hex = center + direction_vector(axis, first_direction) * (radius as i32);
    for i in (first_index + 2)..(first_index + 8) {
        for _ in 0..radius {
            result.push(hex);
            let direction = neighbor_directions[(i % WEDGE_COUNT) as usize];
            hex += direction_vector(axis, direction);
        }
    }
    result
}

pub(crate) fn cube_spiral_for_neighbor(
    axis: TileOffsetAxis,
    neighbor_directions: &[CellNeighbor],
    center: Vector3i,
    radius: i64,
    first_side: i64,
) -> Array<Vector3i> {
    let mut result = Array::<Vector3i>::new();
    result.push(center);
    if radius < 1 {
        return result;
    }

    for k in 1..=radius {
        let mut new_ring = cube_ring_for_neighbor(axis, neighbor_directions, center, k, first_side);
        for _ in 1..k {
            let back = new_ring.pop().unwrap();
            new_ring.push_front(back);
        }
        result.extend_array(&new_ring);
    }
    result
}

pub(crate) fn wedge_index_for_local_direction_for_axis(
    axis: TileOffsetAxis,
    direction: Vector2,
) -> real {
    if axis == TileOffsetAxis::HORIZONTAL {
        (WEDGE_COUNT as real
            - wedge_inv_angle() * (direction.angle_to(Vector2::DOWN) + PI))
            .abs()
    } else {
        (WEDGE_COUNT as real
            - (wedge_inv_angle()
                * (direction.angle_to(Vector2::DOWN.rotated(wedge_angle_half())) + PI)))
            .abs()
    }
}

// ---------------------------------------------------------------------------
// Geometry
// ---------------------------------------------------------------------------

pub(crate) fn geometry_tile_corners(axis: TileOffsetAxis, tile_size: Vector2i) -> Vec<Vector2> {
    let width = tile_size.x as real;
    let height = tile_size.y as real;

    if axis == TileOffsetAxis::HORIZONTAL {
        let half_height = height * 0.5;
        let half_width = width * 0.5;
        let quarter_height = height * 0.25;
        vec![
            Vector2::new(0.0, -half_height),
            Vector2::new(half_width, -quarter_height),
            Vector2::new(half_width, quarter_height),
            Vector2::new(0.0, half_height),
            Vector2::new(-half_width, quarter_height),
            Vector2::new(-half_width, -quarter_height),
        ]
    } else {
        let half_height = height * 0.5;
        let half_width = width * 0.5;
        let quarter_width = (width * 0.25).round();
        vec![
            Vector2::new(quarter_width, -half_height),
            Vector2::new(half_width, 0.0),
            Vector2::new(quarter_width, half_height),
            Vector2::new(-quarter_width, half_height),
            Vector2::new(-half_width, 0.0),
            Vector2::new(-quarter_width, -half_height),
        ]
    }
}

pub(crate) fn geometry_approx_shape(
    axis: TileOffsetAxis,
    tile_size: Vector2i,
) -> (Gd<Shape2D>, Transform2D) {
    let mut shape = CapsuleShape2D::new_gd();
    if axis == TileOffsetAxis::HORIZONTAL {
        if tile_size.y > tile_size.x {
            shape.set_radius(tile_size.x as f32 * 0.5);
            shape.set_height(tile_size.y as f32);
            (shape.upcast::<Shape2D>(), Transform2D::IDENTITY)
        } else {
            shape.set_radius(tile_size.y as f32 * 0.5);
            shape.set_height(tile_size.x as f32);
            (
                shape.upcast::<Shape2D>(),
                Transform2D::IDENTITY.rotated(PI * 0.5),
            )
        }
    } else if tile_size.x > tile_size.y {
        shape.set_radius(tile_size.y as f32 * 0.5);
        shape.set_height(tile_size.x as f32);
        (
            shape.upcast::<Shape2D>(),
            Transform2D::IDENTITY.rotated(PI * 0.5),
        )
    } else {
        shape.set_radius(tile_size.x as f32 * 0.5);
        shape.set_height(tile_size.y as f32);
        (shape.upcast::<Shape2D>(), Transform2D::IDENTITY)
    }
}

// ---------------------------------------------------------------------------
// Godot class
// ---------------------------------------------------------------------------

#[derive(GodotClass)]
#[class(init)]
pub struct HexagonTileMap;

#[godot_api]
impl HexagonTileMap {
    #[func]
    pub fn get_conversion_methods_for(
        axis: TileOffsetAxis,
        layout: TileLayout,
    ) -> Dictionary<GString, Callable> {
        let mut dict = Dictionary::<GString, Callable>::new();
        let (cube_to_map, map_to_cube): (&str, &str) = match (axis, layout) {
            (TileOffsetAxis::HORIZONTAL, TileLayout::STACKED) => {
                ("_cube_to_horizontal_stacked", "_horizontal_stacked_to_cube")
            }
            (TileOffsetAxis::HORIZONTAL, TileLayout::STACKED_OFFSET) => (
                "_cube_to_horizontal_stacked_offset",
                "_horizontal_stacked_offset_to_cube",
            ),
            (TileOffsetAxis::HORIZONTAL, TileLayout::STAIRS_RIGHT) => (
                "_cube_to_horizontal_stairs_right",
                "_horizontal_stairs_right_to_cube",
            ),
            (TileOffsetAxis::HORIZONTAL, TileLayout::STAIRS_DOWN) => (
                "_cube_to_horizontal_stairs_down",
                "_horizontal_stairs_down_to_cube",
            ),
            (TileOffsetAxis::HORIZONTAL, TileLayout::DIAMOND_RIGHT) => (
                "_cube_to_horizontal_diamond_right",
                "_horizontal_diamond_right_to_cube",
            ),
            (TileOffsetAxis::HORIZONTAL, TileLayout::DIAMOND_DOWN) => (
                "_cube_to_horizontal_diamond_down",
                "_horizontal_diamond_down_to_cube",
            ),
            (TileOffsetAxis::VERTICAL, TileLayout::STACKED) => {
                ("_cube_to_vertical_stacked", "_vertical_stacked_to_cube")
            }
            (TileOffsetAxis::VERTICAL, TileLayout::STACKED_OFFSET) => (
                "_cube_to_vertical_stacked_offset",
                "_vertical_stacked_offset_to_cube",
            ),
            (TileOffsetAxis::VERTICAL, TileLayout::STAIRS_RIGHT) => (
                "_cube_to_vertical_stairs_right",
                "_vertical_stairs_right_to_cube",
            ),
            (TileOffsetAxis::VERTICAL, TileLayout::STAIRS_DOWN) => {
                ("_cube_to_vertical_stairs_down", "_vertical_stairs_down_to_cube")
            }
            (TileOffsetAxis::VERTICAL, TileLayout::DIAMOND_RIGHT) => (
                "_cube_to_vertical_diamond_right",
                "_vertical_diamond_right_to_cube",
            ),
            (TileOffsetAxis::VERTICAL, TileLayout::DIAMOND_DOWN) => (
                "_cube_to_vertical_diamond_down",
                "_vertical_diamond_down_to_cube",
            ),
            _ => ("", ""),
        };

        if cube_to_map.is_empty() {
            godot_error!(
                "Could not find conversion method for axis {:?} and layout {:?}",
                axis,
                layout
            );
            return Dictionary::new();
        }

        let cube_to_map_key = GString::from("cube_to_map");
        let cube_to_map_value = Callable::from_class_static("HexagonTileMap", cube_to_map);
        dict.set(&cube_to_map_key, &cube_to_map_value);

        let map_to_cube_key = GString::from("map_to_cube");
        let map_to_cube_value = Callable::from_class_static("HexagonTileMap", map_to_cube);
        dict.set(&map_to_cube_key, &map_to_cube_value);
        dict
    }

    // -- conversion functions (also referenced by the Callable dictionary above) --

    #[func]
    pub fn _cube_to_horizontal_stacked(cube: Vector3i) -> Vector2i {
        cube_to_map_for(TileOffsetAxis::HORIZONTAL, TileLayout::STACKED, cube)
    }
    #[func]
    pub fn _horizontal_stacked_to_cube(map: Vector2i) -> Vector3i {
        map_to_cube_for(TileOffsetAxis::HORIZONTAL, TileLayout::STACKED, map)
    }

    #[func]
    pub fn _cube_to_horizontal_stacked_offset(cube: Vector3i) -> Vector2i {
        cube_to_map_for(TileOffsetAxis::HORIZONTAL, TileLayout::STACKED_OFFSET, cube)
    }
    #[func]
    pub fn _horizontal_stacked_offset_to_cube(map: Vector2i) -> Vector3i {
        map_to_cube_for(TileOffsetAxis::HORIZONTAL, TileLayout::STACKED_OFFSET, map)
    }

    #[func]
    pub fn _cube_to_horizontal_stairs_right(cube: Vector3i) -> Vector2i {
        cube_to_map_for(TileOffsetAxis::HORIZONTAL, TileLayout::STAIRS_RIGHT, cube)
    }
    #[func]
    pub fn _horizontal_stairs_right_to_cube(map: Vector2i) -> Vector3i {
        map_to_cube_for(TileOffsetAxis::HORIZONTAL, TileLayout::STAIRS_RIGHT, map)
    }

    #[func]
    pub fn _cube_to_horizontal_stairs_down(cube: Vector3i) -> Vector2i {
        cube_to_map_for(TileOffsetAxis::HORIZONTAL, TileLayout::STAIRS_DOWN, cube)
    }
    #[func]
    pub fn _horizontal_stairs_down_to_cube(map: Vector2i) -> Vector3i {
        map_to_cube_for(TileOffsetAxis::HORIZONTAL, TileLayout::STAIRS_DOWN, map)
    }

    #[func]
    pub fn _cube_to_horizontal_diamond_right(cube: Vector3i) -> Vector2i {
        cube_to_map_for(TileOffsetAxis::HORIZONTAL, TileLayout::DIAMOND_RIGHT, cube)
    }
    #[func]
    pub fn _horizontal_diamond_right_to_cube(map: Vector2i) -> Vector3i {
        map_to_cube_for(TileOffsetAxis::HORIZONTAL, TileLayout::DIAMOND_RIGHT, map)
    }

    #[func]
    pub fn _cube_to_horizontal_diamond_down(cube: Vector3i) -> Vector2i {
        cube_to_map_for(TileOffsetAxis::HORIZONTAL, TileLayout::DIAMOND_DOWN, cube)
    }
    #[func]
    pub fn _horizontal_diamond_down_to_cube(map: Vector2i) -> Vector3i {
        map_to_cube_for(TileOffsetAxis::HORIZONTAL, TileLayout::DIAMOND_DOWN, map)
    }

    #[func]
    pub fn _cube_to_vertical_stacked(cube: Vector3i) -> Vector2i {
        cube_to_map_for(TileOffsetAxis::VERTICAL, TileLayout::STACKED, cube)
    }
    #[func]
    pub fn _vertical_stacked_to_cube(map: Vector2i) -> Vector3i {
        map_to_cube_for(TileOffsetAxis::VERTICAL, TileLayout::STACKED, map)
    }

    #[func]
    pub fn _cube_to_vertical_stacked_offset(cube: Vector3i) -> Vector2i {
        cube_to_map_for(TileOffsetAxis::VERTICAL, TileLayout::STACKED_OFFSET, cube)
    }
    #[func]
    pub fn _vertical_stacked_offset_to_cube(map: Vector2i) -> Vector3i {
        map_to_cube_for(TileOffsetAxis::VERTICAL, TileLayout::STACKED_OFFSET, map)
    }

    #[func]
    pub fn _cube_to_vertical_stairs_right(cube: Vector3i) -> Vector2i {
        cube_to_map_for(TileOffsetAxis::VERTICAL, TileLayout::STAIRS_RIGHT, cube)
    }
    #[func]
    pub fn _vertical_stairs_right_to_cube(map: Vector2i) -> Vector3i {
        map_to_cube_for(TileOffsetAxis::VERTICAL, TileLayout::STAIRS_RIGHT, map)
    }

    #[func]
    pub fn _cube_to_vertical_stairs_down(cube: Vector3i) -> Vector2i {
        cube_to_map_for(TileOffsetAxis::VERTICAL, TileLayout::STAIRS_DOWN, cube)
    }
    #[func]
    pub fn _vertical_stairs_down_to_cube(map: Vector2i) -> Vector3i {
        map_to_cube_for(TileOffsetAxis::VERTICAL, TileLayout::STAIRS_DOWN, map)
    }

    #[func]
    pub fn _cube_to_vertical_diamond_right(cube: Vector3i) -> Vector2i {
        cube_to_map_for(TileOffsetAxis::VERTICAL, TileLayout::DIAMOND_RIGHT, cube)
    }
    #[func]
    pub fn _vertical_diamond_right_to_cube(map: Vector2i) -> Vector3i {
        map_to_cube_for(TileOffsetAxis::VERTICAL, TileLayout::DIAMOND_RIGHT, map)
    }

    #[func]
    pub fn _cube_to_vertical_diamond_down(cube: Vector3i) -> Vector2i {
        cube_to_map_for(TileOffsetAxis::VERTICAL, TileLayout::DIAMOND_DOWN, cube)
    }
    #[func]
    pub fn _vertical_diamond_down_to_cube(map: Vector2i) -> Vector3i {
        map_to_cube_for(TileOffsetAxis::VERTICAL, TileLayout::DIAMOND_DOWN, map)
    }

    // -- public cube math API --

    #[func]
    pub fn cube_direction_for_axis(axis: TileOffsetAxis, direction: CellNeighbor) -> Vector3i {
        direction_vector(axis, direction)
    }

    #[func]
    pub fn cube_neighbor_for_axis(
        axis: TileOffsetAxis,
        cube: Vector3i,
        direction: CellNeighbor,
    ) -> Vector3i {
        cube + direction_vector(axis, direction)
    }

    #[func]
    pub fn cube_neighbors_for_axis(axis: TileOffsetAxis, cube: Vector3i) -> Array<Vector3i> {
        side_directions(axis)
            .into_iter()
            .map(|direction| cube + direction_vector(axis, direction))
            .collect()
    }

    #[func]
    pub fn cube_corner_neighbors_for_axis(axis: TileOffsetAxis, cube: Vector3i) -> Array<Vector3i> {
        corner_directions(axis)
            .into_iter()
            .map(|direction| cube + direction_vector(axis, direction))
            .collect()
    }

    #[func]
    pub fn cube_distance(a: Vector3i, b: Vector3i) -> i64 {
        cube_distance(a, b)
    }

    #[func]
    pub fn cube_round(frac: Vector3) -> Vector3i {
        cube_round(frac)
    }

    #[func]
    pub fn cube_linedraw(a: Vector3i, b: Vector3i) -> Array<Vector3i> {
        cube_linedraw(a, b)
    }

    #[func]
    pub fn cube_range(center: Vector3i, distance: i64) -> Array<Vector3i> {
        cube_range(center, distance)
    }

    #[func]
    pub fn cube_intersect_ranges(
        center1: Vector3i,
        range1: i64,
        center2: Vector3i,
        range2: i64,
    ) -> Array<Vector3i> {
        cube_intersect_ranges(center1, range1, center2, range2)
    }

    #[func]
    pub fn cube_rotate(position: Vector3i, rotations: i64) -> Vector3i {
        cube_rotate(position, rotations)
    }

    #[func]
    pub fn cube_rotate_from(position: Vector3i, from: Vector3i, rotations: i64) -> Vector3i {
        cube_rotate_from(position, from, rotations)
    }

    #[func]
    pub fn cube_reflect(position: Vector3i, axis: i64) -> Vector3i {
        cube_reflect(position, axis)
    }

    #[func]
    pub fn cube_reflect_from(position: Vector3i, from: Vector3i, axis: i64) -> Vector3i {
        cube_reflect_from(position, from, axis)
    }

    #[func]
    pub fn cube_rect(
        center: Vector3i,
        corner: Vector3i,
        #[opt(default = 1)] axis: i64,
    ) -> Array<Vector3i> {
        cube_rect(center, corner, axis)
    }

    #[func]
    pub fn cube_rect_corners(
        center: Vector3i,
        corner: Vector3i,
        #[opt(default = 1)] axis: i64,
    ) -> Array<Vector3i> {
        cube_rect_corners(center, corner, axis)
    }

    #[func]
    pub fn cube_ring_for_axis(
        axis: TileOffsetAxis,
        center: Vector3i,
        radius: i64,
        #[opt(default = -1)] first_side: i64,
    ) -> Array<Vector3i> {
        let side = side_directions(axis);
        cube_ring_for_neighbor(axis, &side, center, radius, first_side)
    }

    #[func]
    pub fn cube_spiral_for_axis(
        axis: TileOffsetAxis,
        center: Vector3i,
        radius: i64,
        #[opt(default = -1)] first_side: i64,
    ) -> Array<Vector3i> {
        let side = side_directions(axis);
        cube_spiral_for_neighbor(axis, &side, center, radius, first_side)
    }

    #[func]
    pub fn get_geometry_methods_for(axis: TileOffsetAxis) -> Dictionary<GString, Callable> {
        let mut dict = Dictionary::<GString, Callable>::new();
        let name = if axis == TileOffsetAxis::HORIZONTAL {
            "geometry_horizontal_tile_corners"
        } else {
            "geometry_vertical_tile_corners"
        };
        let key = GString::from("tile_corners");
        let value = Callable::from_class_static("HexagonTileMap", name);
        dict.set(&key, &value);
        dict
    }

    #[func]
    pub fn geometry_horizontal_tile_corners(tile_size: Vector2i) -> Array<Vector2> {
        geometry_tile_corners(TileOffsetAxis::HORIZONTAL, tile_size)
            .iter()
            .copied()
            .collect()
    }

    #[func]
    pub fn geometry_vertical_tile_corners(tile_size: Vector2i) -> Array<Vector2> {
        geometry_tile_corners(TileOffsetAxis::VERTICAL, tile_size)
            .iter()
            .copied()
            .collect()
    }

    #[func]
    pub fn update_cells_layout(
        mut tilemap: Gd<HexagonTileMapLayer>,
        from: TileLayout,
        to: TileLayout,
    ) {
        let mut layer = tilemap.bind_mut();
        let Some(mut tile_set) = layer.base().get_tile_set() else {
            return;
        };
        let axis = tile_set.get_tile_offset_axis();

        let mut tiles: Vec<(Vector3i, i32, Vector2i, i32)> = Vec::new();
        let used_cells = layer.base().get_used_cells();
        for pos in used_cells.iter_shared() {
            let cube = map_to_cube_for(axis, from, pos);
            let source = layer.base().get_cell_source_id(pos);
            let atlas = layer.base().get_cell_atlas_coords(pos);
            let alternative = layer.base().get_cell_alternative_tile(pos);
            tiles.push((cube, source, atlas, alternative));
            layer.base_mut().erase_cell(pos);
        }

        tile_set.set_tile_layout(to);

        for (cube, source, atlas, alternative) in tiles {
            let map = cube_to_map_for(axis, to, cube);
            layer
                .base_mut()
                .set_cell_ex(map)
                .source_id(source)
                .atlas_coords(atlas)
                .alternative_tile(alternative)
                .done();
        }
    }
}
