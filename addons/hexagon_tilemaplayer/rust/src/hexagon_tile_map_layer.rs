//! Rust conversion of `hexagon_tilemaplayer.gd`.
//!
//! [`HexagonTileMapLayer`] is a `TileMapLayer` subclass providing hexagon tile
//! conversion helpers, AStar2D pathfinding and debug visualization.

use std::collections::{HashMap, HashSet, VecDeque};

use godot::builtin::real;
use godot::classes::control::MouseFilter;
use godot::classes::object::ConnectFlags;
use godot::classes::tile_set::{CellNeighbor, TileLayout, TileOffsetAxis, TileShape};
use godot::classes::{
    AStar2D, ConvexPolygonShape2D, Engine, ITileMapLayer, Line2D, Node2D, RichTextLabel, Shape2D,
    TileMapLayer, TileSet,
};
use godot::prelude::*;

use crate::hexagon_tile_map as tile_map_math;
use crate::hexagon_tile_map::HexagonTileMap;

// ---------------------------------------------------------------------------
// TileShape helper
// ---------------------------------------------------------------------------

/// Shape of a single tile, based on a [`Shape2D`] and a [`Transform2D`].
///
/// This is the Rust equivalent of the `TileShape` inner class in the original
/// GDScript. Godot's GDExtension API does not support nested classes, so it is
/// exposed as a top-level class.
#[derive(GodotClass)]
#[class(init, base = RefCounted)]
pub struct HexagonTileShape {
    #[var]
    shape: Option<Gd<Shape2D>>,

    #[var]
    #[init(val = Transform2D::IDENTITY)]
    transform: Transform2D,

    base: Base<RefCounted>,
}

#[godot_api]
impl HexagonTileShape {
    /// Returns a copy of the tile shape with a new position.
    #[func]
    pub fn get_for(&self, position: Vector2) -> Gd<HexagonTileShape> {
        let transform = Transform2D::from_cols(self.transform.a, self.transform.b, position);
        HexagonTileShape::from_shape_transform(self.shape.clone(), transform)
    }
}

impl HexagonTileShape {
    pub(crate) fn from_shape_transform(
        shape: Option<Gd<Shape2D>>,
        transform: Transform2D,
    ) -> Gd<Self> {
        Gd::from_init_fn(|base| Self {
            shape,
            transform,
            base,
        })
    }
}

// ---------------------------------------------------------------------------
// Debug mode flags
// ---------------------------------------------------------------------------

#[allow(dead_code)]
enum DebugModeFlags {
    TilesCoords = 1,
    Connections = 2,
}

#[derive(Clone, PartialEq)]
struct TileDataKey {
    tile_set_id: InstanceId,
    axis: TileOffsetAxis,
    layout: TileLayout,
    tile_size: Vector2i,
}

fn call_bool(callable: &Callable, arg: Vector3i) -> bool {
    if !callable.is_valid() {
        return false;
    }
    let mut args = Array::<Variant>::new();
    let variant = Variant::from(arg);
    args.push(&variant);
    callable.callv(&args).to::<bool>()
}

fn debug_color(seed: u64) -> Color {
    let mut x = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    let r = ((x >> 32) & 0xFF) as u8;
    let g = ((x >> 16) & 0xFF) as u8;
    let b = (x & 0xFF) as u8;
    Color::from_rgba8(r, g, b, 255)
}

// ---------------------------------------------------------------------------
// HexagonTileMapLayer
// ---------------------------------------------------------------------------

#[derive(GodotClass)]
#[class(tool, base = TileMapLayer)]
pub struct HexagonTileMapLayer {
    base: Base<TileMapLayer>,

    #[export]
    pathfinding_enabled: bool,

    #[export(flags = (Tiles_coords, Connections))]
    #[var(set = set_debug_mode)]
    debug_mode: i64,

    #[var]
    astar: Option<Gd<AStar2D>>,

    #[var]
    geometry_tile_shape: Option<Gd<HexagonTileShape>>,

    #[var]
    geometry_tile_approx_shape: Option<Gd<HexagonTileShape>>,

    #[var]
    cube_direction_vectors: Dictionary<CellNeighbor, Vector3i>,

    #[var]
    cube_side_neighbor_directions: Array<CellNeighbor>,

    #[var]
    cube_corner_neighbor_directions: Array<CellNeighbor>,

    debug_container: Option<Gd<Node2D>>,
    debug_font_size: i64,
    debug_font_outline_size: i64,

    cache_key: Option<TileDataKey>,
    side_dirs: Vec<CellNeighbor>,
    corner_dirs: Vec<CellNeighbor>,
    dir_vectors: HashMap<CellNeighbor, Vector3i>,
}

#[godot_api]
impl ITileMapLayer for HexagonTileMapLayer {
    fn init(base: Base<TileMapLayer>) -> Self {
        Self {
            base,
            pathfinding_enabled: false,
            debug_mode: 0,
            astar: None,
            geometry_tile_shape: None,
            geometry_tile_approx_shape: None,
            cube_direction_vectors: Dictionary::default(),
            cube_side_neighbor_directions: Array::new(),
            cube_corner_neighbor_directions: Array::new(),
            debug_container: None,
            debug_font_size: 12,
            debug_font_outline_size: 2,
            cache_key: None,
            side_dirs: Vec::new(),
            corner_dirs: Vec::new(),
            dir_vectors: HashMap::new(),
        }
    }

    fn ready(&mut self) {
        if Engine::singleton().is_editor_hint() {
            return;
        }

        self.ensure_tile_data();

        if self.pathfinding_enabled {
            self.pathfinding_generate_points();
        } else {
            self.base_mut().call_deferred("_draw_debug", &[]);
        }
    }

    fn get_configuration_warnings(&self) -> PackedStringArray {
        let mut warnings = PackedStringArray::new();
        match self.base().get_tile_set() {
            None => {
                let warning = GString::from("No tile_set defined.");
                warnings.push(&warning);
            }
            Some(tile_set) => {
                if tile_set.get_tile_shape() != TileShape::HEXAGON {
                    let warning = GString::from("This node only support hexagon shapes.");
                    warnings.push(&warning);
                }
            }
        }
        warnings
    }
}

#[godot_api]
impl HexagonTileMapLayer {
    #[signal]
    pub fn astar_changed();

    // -----------------------------------------------------------------------
    // Properties
    // -----------------------------------------------------------------------

    #[func]
    pub fn set_debug_mode(&mut self, value: i64) {
        self.debug_mode = value;
        if !Engine::singleton().is_editor_hint() && self.base().is_inside_tree() {
            self._draw_debug();
        }
    }

    // -----------------------------------------------------------------------
    // Pathfinding
    // -----------------------------------------------------------------------

    #[func(virtual)]
    pub fn pathfinding_get_tile_weight(&self, _coords: Vector2i) -> f64 {
        1.0
    }

    #[func(virtual)]
    pub fn pathfinding_does_tile_connect(&self, _tile: Vector2i, _neighbor: Vector2i) -> bool {
        true
    }

    #[func]
    pub fn pathfinding_get_point_id(&self, coord: Vector2i) -> i64 {
        let Some(astar) = &self.astar else {
            return -1;
        };
        astar.get_closest_point(self.base().map_to_local(coord))
    }

    #[func]
    pub fn pathfinding_recalculate_tile_weight(&mut self, coord: Vector2i) {
        let weight = self.pathfinding_get_tile_weight(coord);
        let id = self.pathfinding_get_point_id(coord);
        if let Some(astar) = &mut self.astar {
            astar.set_point_weight_scale(id, weight as f32);
        }
    }

    #[func]
    pub fn _pathfinding_generate_points(&mut self) {
        self.pathfinding_generate_points();
    }

    #[func]
    pub fn pathfinding_generate_points(&mut self) {
        if self.astar.is_none() {
            self.astar = Some(AStar2D::new_gd());
        }

        self.ensure_tile_data();
        self._pathfinding_create_points();
        self._pathfinding_create_connections();

        self.signals().astar_changed().emit();
        self.base_mut().call_deferred("_draw_debug", &[]);
    }

    fn _pathfinding_create_points(&mut self) {
        let mut astar_opt = self.astar.clone();
        if let Some(astar) = astar_opt.as_mut() {
            astar.clear();

            let cells = self.base().get_used_cells();
            // Why 1.12? See https://github.com/godotengine/godot/issues/102612#issuecomment-2702275926
            let needed_space = (cells.len() as f32 * 1.12) as i64;
            if astar.get_point_capacity() < needed_space {
                astar.reserve_space(needed_space);
            }

            let mut id: i64 = -1;
            for coord in cells.iter_shared() {
                id += 1;
                let weight = self.pathfinding_get_tile_weight(coord);
                let position = self.base().map_to_local(coord);
                astar
                    .add_point_ex(id, position)
                    .weight_scale(weight as f32)
                    .done();
            }
        }

        self.astar = astar_opt;
    }

    fn _pathfinding_create_connections(&mut self) {
        self.ensure_tile_data();

        let mut astar_opt = self.astar.clone();
        if let Some(astar) = astar_opt.as_mut() {
            let side_dirs = self.side_dirs.clone();
            let neighbours = &side_dirs[..3];

            let ids = astar.get_point_ids();
            for id in ids.as_slice() {
                let local_position = astar.get_point_position(*id);
                let map_position = self.base().local_to_map(local_position);

                for neighbour in neighbours {
                    let neighbour_map_position =
                        self.base().get_neighbor_cell(map_position, *neighbour);

                    if self.base().get_cell_source_id(neighbour_map_position) == -1 {
                        continue;
                    }
                    if !self.pathfinding_does_tile_connect(map_position, neighbour_map_position) {
                        continue;
                    }

                    let neighbour_id =
                        astar.get_closest_point(self.base().map_to_local(neighbour_map_position));
                    astar.connect_points(*id, neighbour_id);
                }
            }
        }

        self.astar = astar_opt;
    }

    // -----------------------------------------------------------------------
    // Debug drawing
    // -----------------------------------------------------------------------

    #[func]
    pub fn _draw_debug(&mut self) {
        if let Some(mut container) = self.debug_container.take() {
            if container.is_instance_valid() {
                container.queue_free();
            }
        }

        if self.debug_mode == 0 || !self.base().is_visible_in_tree() {
            return;
        }

        let mut container = Node2D::new_alloc();
        container.set_z_index(100);

        if (self.debug_mode & DebugModeFlags::Connections as i64) != 0 && self.pathfinding_enabled
        {
            let mut connections_container = Node2D::new_alloc();
            let astar = self.astar.clone();
            if let Some(astar) = astar {
                let ids = astar.get_point_ids();
                for id in ids.as_slice() {
                    let connections = astar.get_point_connections(*id);
                    for neighbour_id in connections.as_slice() {
                        if *neighbour_id > *id {
                            let seed = (*id as u64) ^ ((*neighbour_id as u64) << 32);
                            self._pathfinding_debug_display_connection(
                                &mut connections_container,
                                *id,
                                *neighbour_id,
                                debug_color(seed),
                            );
                        }
                    }
                }
            }
            container.add_child(&connections_container);
        }

        if (self.debug_mode & DebugModeFlags::TilesCoords as i64) != 0 {
            let mut coords_container = Node2D::new_alloc();
            if self.pathfinding_enabled {
                let astar = self.astar.clone();
                if let Some(astar) = astar {
                    let ids = astar.get_point_ids();
                    for id in ids.as_slice() {
                        self._debug_tile_coords_with_pathfinding(&mut coords_container, *id);
                    }
                }
            } else {
                let cells = self.base().get_used_cells();
                for pos in cells.iter_shared() {
                    self._debug_tile_coords(&mut coords_container, pos);
                }
            }
            container.add_child(&coords_container);
        }

        self.debug_container = Some(container.clone());
        self.base_mut().add_child(&container);
    }

    fn _pathfinding_debug_display_connection(
        &self,
        debug_container: &mut Node2D,
        id1: i64,
        id2: i64,
        color: Color,
    ) {
        let mut line = Line2D::new_alloc();
        line.set_default_color(color);
        if let Some(astar) = &self.astar {
            line.add_point(astar.get_point_position(id1));
            line.add_point(astar.get_point_position(id2));
        }
        debug_container.add_child(&line);
    }

    fn _debug_tile_coords_with_pathfinding(&mut self, debug_container: &mut Node2D, id: i64) {
        let astar = self.astar.clone();
        let Some(astar) = astar else {
            return;
        };

        let pos = self.base().local_to_map(astar.get_point_position(id));
        let cube = self.map_to_cube(pos);
        let text = format!(
            "[center]#{}\n({}, {})\n({}, {}, {})[/center]",
            id, pos.x, pos.y, cube.x, cube.y, cube.z
        );
        self._show_debug_text_on_tile(debug_container, pos, GString::from(text.as_str()));
    }

    fn _debug_tile_coords(&mut self, debug_container: &mut Node2D, pos: Vector2i) {
        let cube = self.map_to_cube(pos);
        let text = format!(
            "[center]({}, {})\n({}, {}, {})[/center]",
            pos.x, pos.y, cube.x, cube.y, cube.z
        );
        self._show_debug_text_on_tile(debug_container, pos, GString::from(text.as_str()));
    }

    fn _show_debug_text_on_tile(&self, debug_container: &mut Node2D, pos: Vector2i, text: GString) {
        let mut label = RichTextLabel::new_alloc();
        label.set_fit_content(true);
        label.set_position(self.base().map_to_local(pos));
        label.set_use_bbcode(true);
        label.set_text(&text);

        let width = self
            .base()
            .get_tile_set()
            .map(|tile_set| tile_set.get_tile_size().x as f32)
            .unwrap_or(12.0);
        label.set_size(Vector2::new(width, 0.0));

        let label_id = label.instance_id();
        label
            .signals()
            .resized()
            .builder()
            .flags(ConnectFlags::ONE_SHOT)
            .connect(move || {
                if let Ok(mut label) = Gd::<RichTextLabel>::try_from_instance_id(label_id) {
                    let size = label.get_size();
                    let mut position = label.get_position();
                    position.x -= size.x * 0.5;
                    position.y -= size.y * 0.5;
                    label.set_position(position);
                }
            });

        label.set_mouse_filter(MouseFilter::IGNORE);
        label.set_scroll_active(false);
        label.add_theme_font_size_override("normal_font_size", self.debug_font_size as i32);
        label.add_theme_constant_override("outline_size", self.debug_font_outline_size as i32);
        debug_container.add_child(&label);
    }

    // -----------------------------------------------------------------------
    // Tile data cache
    // -----------------------------------------------------------------------

    fn ensure_tile_data(&mut self) {
        let Some(tile_set) = self.base().get_tile_set() else {
            if self.cache_key.is_some() {
                self.cache_key = None;
                self.clear_tile_data();
            }
            return;
        };

        let key = TileDataKey {
            tile_set_id: tile_set.instance_id(),
            axis: tile_set.get_tile_offset_axis(),
            layout: tile_set.get_tile_layout(),
            tile_size: tile_set.get_tile_size(),
        };

        if self.cache_key.as_ref() != Some(&key) {
            self.cache_key = Some(key.clone());
            self.rebuild_tile_data(&tile_set);
        }
    }

    fn clear_tile_data(&mut self) {
        self.debug_font_size = 12;
        self.debug_font_outline_size = 2;
        self.cube_direction_vectors.clear();
        self.cube_side_neighbor_directions.clear();
        self.cube_corner_neighbor_directions.clear();
        self.side_dirs.clear();
        self.corner_dirs.clear();
        self.dir_vectors.clear();
        self.geometry_tile_shape = None;
        self.geometry_tile_approx_shape = None;
    }

    fn rebuild_tile_data(&mut self, tile_set: &Gd<TileSet>) {
        let tile_size = tile_set.get_tile_size();
        self.debug_font_size = ((tile_size.x as f32) / 7.0).floor() as i64;
        self.debug_font_outline_size = ((tile_size.x as f32) / 32.0).floor() as i64;

        let axis = tile_set.get_tile_offset_axis();
        self.side_dirs = tile_map_math::side_directions(axis);
        self.corner_dirs = tile_map_math::corner_directions(axis);
        self.dir_vectors = tile_map_math::direction_vectors(axis);

        self.cube_side_neighbor_directions = Array::from(self.side_dirs.as_slice());
        self.cube_corner_neighbor_directions = Array::from(self.corner_dirs.as_slice());
        self.cube_direction_vectors.clear();
        for (direction, vector) in &self.dir_vectors {
            self.cube_direction_vectors.set(*direction, *vector);
        }

        let corners = tile_map_math::geometry_tile_corners(axis, tile_size);
        let mut points = PackedVector2Array::new();
        for corner in &corners {
            points.push(*corner);
        }
        let mut convex = ConvexPolygonShape2D::new_gd();
        convex.set_points(&points);
        let convex: Gd<Shape2D> = convex.upcast::<Shape2D>();
        self.geometry_tile_shape = Some(HexagonTileShape::from_shape_transform(
            Some(convex),
            Transform2D::IDENTITY,
        ));

        let (approx_shape, approx_transform) =
            tile_map_math::geometry_approx_shape(axis, tile_size);
        self.geometry_tile_approx_shape = Some(HexagonTileShape::from_shape_transform(
            Some(approx_shape),
            approx_transform,
        ));
    }

    // -----------------------------------------------------------------------
    // Cube <-> map conversions
    // -----------------------------------------------------------------------

    #[func]
    pub fn cube_to_map(&mut self, cube_position: Vector3i) -> Vector2i {
        self.ensure_tile_data();
        let Some(key) = &self.cache_key else {
            godot_error!(
                "Could not call cube_to_map because the conversion method was not set up. Make sure the tile_set is defined."
            );
            return Vector2i::ZERO;
        };
        tile_map_math::cube_to_map_for(key.axis, key.layout, cube_position)
    }

    #[func]
    pub fn map_to_cube(&mut self, map_position: Vector2i) -> Vector3i {
        self.ensure_tile_data();
        let Some(key) = &self.cache_key else {
            godot_error!(
                "Could not call map_to_cube because the conversion method was not set up. Make sure the tile_set is defined."
            );
            return Vector3i::ZERO;
        };
        tile_map_math::map_to_cube_for(key.axis, key.layout, map_position)
    }

    #[func]
    pub fn cube_to_local(&mut self, cube_position: Vector3i) -> Vector2 {
        let map = self.cube_to_map(cube_position);
        self.base().map_to_local(map)
    }

    #[func]
    pub fn local_to_cube(&mut self, map_position: Vector2) -> Vector3i {
        let map = self.base().local_to_map(map_position);
        self.map_to_cube(map)
    }

    // -----------------------------------------------------------------------
    // Cube math
    // -----------------------------------------------------------------------

    #[func]
    pub fn cube_direction(&mut self, direction: CellNeighbor) -> Vector3i {
        self.ensure_tile_data();
        match &self.cache_key {
            Some(key) => tile_map_math::direction_vector(key.axis, direction),
            None => Vector3i::ZERO,
        }
    }

    #[func]
    pub fn cube_neighbor(&mut self, cube: Vector3i, direction: CellNeighbor) -> Vector3i {
        cube + self.cube_direction(direction)
    }

    #[func]
    pub fn cube_neighbors(&mut self, cube: Vector3i) -> Array<Vector3i> {
        self.ensure_tile_data();
        self.side_dirs
            .iter()
            .map(|direction| cube + self.dir_vectors[direction])
            .collect()
    }

    #[func]
    pub fn cube_corner_neighbors(&mut self, cube: Vector3i) -> Array<Vector3i> {
        self.ensure_tile_data();
        self.corner_dirs
            .iter()
            .map(|direction| cube + self.dir_vectors[direction])
            .collect()
    }

    #[func]
    pub fn cube_distance(&self, a: Vector3i, b: Vector3i) -> i64 {
        tile_map_math::cube_distance(a, b)
    }

    #[func]
    pub fn cube_round(&self, frac: Vector3) -> Vector3i {
        tile_map_math::cube_round(frac)
    }

    #[func]
    pub fn cube_linedraw(&self, a: Vector3i, b: Vector3i) -> Array<Vector3i> {
        tile_map_math::cube_linedraw(a, b)
    }

    #[func]
    pub fn cube_range(&self, center: Vector3i, distance: i64) -> Array<Vector3i> {
        tile_map_math::cube_range(center, distance)
    }

    #[func]
    pub fn cube_intersect_ranges(
        &self,
        center1: Vector3i,
        range1: i64,
        center2: Vector3i,
        range2: i64,
    ) -> Array<Vector3i> {
        tile_map_math::cube_intersect_ranges(center1, range1, center2, range2)
    }

    #[func]
    pub fn cube_rotate(&self, cell: Vector3i, rotations: i64) -> Vector3i {
        tile_map_math::cube_rotate(cell, rotations)
    }

    #[func]
    pub fn cube_rotate_from(&self, cell: Vector3i, from: Vector3i, rotations: i64) -> Vector3i {
        tile_map_math::cube_rotate_from(cell, from, rotations)
    }

    #[func]
    pub fn cube_reflect(&self, cell: Vector3i, axis: i64) -> Vector3i {
        tile_map_math::cube_reflect(cell, axis)
    }

    #[func]
    pub fn cube_reflect_from(&self, cell: Vector3i, from: Vector3i, axis: i64) -> Vector3i {
        tile_map_math::cube_reflect_from(cell, from, axis)
    }

    #[func]
    pub fn cube_rect(
        &self,
        center: Vector3i,
        corner: Vector3i,
        #[opt(default = 1)] axis: i64,
    ) -> Array<Vector3i> {
        tile_map_math::cube_rect(center, corner, axis)
    }

    #[func]
    pub fn cube_rect_corners(
        &self,
        center: Vector3i,
        corner: Vector3i,
        #[opt(default = 1)] axis: i64,
    ) -> Array<Vector3i> {
        tile_map_math::cube_rect_corners(center, corner, axis)
    }

    #[func]
    pub fn cube_ring(
        &mut self,
        center: Vector3i,
        radius: i64,
        #[opt(default = -1)] first_side: i64,
    ) -> Array<Vector3i> {
        self.ensure_tile_data();
        tile_map_math::cube_ring_for_neighbor(
            self.axis(),
            &self.side_dirs,
            center,
            radius,
            first_side,
        )
    }

    #[func]
    pub fn cube_spiral(
        &mut self,
        center: Vector3i,
        radius: i64,
        #[opt(default = -1)] first_side: i64,
    ) -> Array<Vector3i> {
        self.ensure_tile_data();
        tile_map_math::cube_spiral_for_neighbor(
            self.axis(),
            &self.side_dirs,
            center,
            radius,
            first_side,
        )
    }

    #[func]
    pub fn cube_explore(
        &mut self,
        start: Vector3i,
        filter: Callable,
        maybe: Callable,
    ) -> Array<Vector3i> {
        self.ensure_tile_data();

        let mut result = Array::<Vector3i>::new();
        let mut visited = HashSet::<Vector3i>::new();
        let mut queue = VecDeque::<Vector3i>::new();

        queue.push_back(start);
        visited.insert(start);
        if call_bool(&filter, start) {
            result.push(start);
        }

        while let Some(current) = queue.pop_front() {
            let neighbours = self.cube_neighbors(current);
            for neighbour in neighbours.iter_shared() {
                if visited.contains(&neighbour) {
                    continue;
                }
                visited.insert(neighbour);

                let is_filtered = call_bool(&filter, neighbour);
                if is_filtered {
                    result.push(neighbour);
                }

                if is_filtered || call_bool(&maybe, neighbour) {
                    queue.push_back(neighbour);
                }
            }
        }

        result
    }

    // -----------------------------------------------------------------------
    // Closest cell operations
    // -----------------------------------------------------------------------

    #[func]
    pub fn get_closest_cell_from_mouse(&mut self) -> Vector3i {
        let local = self.base().get_local_mouse_position();
        self.get_closest_cell_from_local(local)
    }

    #[func]
    pub fn get_closest_cell_from_local(&mut self, local: Vector2) -> Vector3i {
        let map = self.base().local_to_map(local);
        self.map_to_cube(map)
    }

    #[func]
    pub fn get_closest_cells_from_mouse(
        &mut self,
        #[opt(default = 1)] count: i64,
    ) -> Array<Vector3i> {
        let local = self.base().get_local_mouse_position();
        self.get_closest_cells_from_local(local, count)
    }

    #[func]
    pub fn get_closest_cells_from_local(
        &mut self,
        local: Vector2,
        #[opt(default = 1)] count: i64,
    ) -> Array<Vector3i> {
        self.ensure_tile_data();

        let center_map = self.base().local_to_map(local);
        let center_cube = self.map_to_cube(center_map);
        let mut cells = Array::<Vector3i>::new();
        cells.push(center_cube);

        if count == 1 {
            return cells;
        }
        cells.resize(count as usize, Vector3i::ZERO);
        let mut remaining = count - 1;

        let axis = self.axis();
        let center_local = self.base().map_to_local(center_map);
        let wedge_index = tile_map_math::wedge_index_for_local_direction_for_axis(
            axis,
            local - center_local,
        );
        let direction_index = wedge_index.ceil() as i64;
        let next_after = wedge_index - direction_index as real > -0.5;
        let index = ((direction_index - 1).rem_euclid(6)) as usize;
        let direction = self.side_dirs[index];
        let mut current_ring_size: i64 = 1;

        if next_after {
            while remaining > 0 {
                let ring = self.cube_ring(center_cube, current_ring_size, direction.ord() as i64);
                current_ring_size += 1;
                let half = ring.len() / 2 + 1;
                for i in 0..half {
                    cells.push(ring.get(i).unwrap());
                    remaining -= 1;
                    if remaining < 0 {
                        return cells;
                    }

                    let neg = if i == 0 { 0 } else { ring.len() - i };
                    cells.push(ring.get(neg).unwrap());
                    remaining -= 1;
                    if remaining < 0 {
                        return cells;
                    }
                }
            }
        } else {
            while remaining > 0 {
                let ring = self.cube_ring(center_cube, current_ring_size, direction.ord() as i64);
                current_ring_size += 1;
                let half = ring.len() / 2 + 1;
                for i in 0..half {
                    let neg = if i == 0 { 0 } else { ring.len() - i };
                    cells.push(ring.get(neg).unwrap());
                    remaining -= 1;
                    if remaining < 0 {
                        return cells;
                    }

                    cells.push(ring.get(i).unwrap());
                    remaining -= 1;
                    if remaining < 0 {
                        return cells;
                    }
                }
            }
        }

        Array::new()
    }

    #[func]
    pub fn cube_outlines(&mut self, cells: Array<Vector3i>) -> Array<Variant> {
        self.ensure_tile_data();

        let Some(key) = self.cache_key.clone() else {
            return Array::new();
        };
        let corners = tile_map_math::geometry_tile_corners(key.axis, key.tile_size);
        let side_dirs = self.side_dirs.clone();

        // Step 1: generate all edge segments.
        let mut edges: Vec<[Vector2; 2]> = Vec::new();
        for cell in cells.iter_shared() {
            let cell_center = self.cube_to_local(cell);
            for i in 0..6 {
                let neighbor = self.cube_neighbor(cell, side_dirs[i]);
                if !cells.contains(neighbor) {
                    edges.push([
                        cell_center + corners[i],
                        cell_center + corners[(i + 1) % 6],
                    ]);
                }
            }
        }

        // Step 2: chain the segments into outlines.
        let mut outlines = Array::<Variant>::new();
        while !edges.is_empty() {
            let mut current_outline: Vec<Vector2> = Vec::new();
            let start_edge = edges.remove(0);
            let mut next_point = start_edge[1];
            current_outline.push(start_edge[0]);

            let safety = edges.len();
            for _ in 0..=safety {
                current_outline.push(next_point);

                if next_point.distance_squared_to(current_outline[0]) < 1.0 {
                    current_outline.pop();
                    break;
                }

                let mut found = false;
                for i in 0..edges.len() {
                    let edge = edges[i];
                    if next_point.distance_squared_to(edge[0]) < 1.0 {
                        next_point = edge[1];
                        edges.remove(i);
                        found = true;
                        break;
                    } else if next_point.distance_squared_to(edge[1]) < 1.0 {
                        next_point = edge[0];
                        edges.remove(i);
                        found = true;
                        break;
                    }
                }

                if !found {
                    godot_error!("Could not find the next edge in the loop");
                    break;
                }
            }

            if current_outline.len() > 2 {
                let inner = Array::<Vector2>::from(current_outline.as_slice());
                let inner_variant = Variant::from(inner);
                outlines.push(&inner_variant);
            }
        }

        outlines
    }

    #[func]
    pub fn update_cells_layout(&mut self, from: TileLayout, to: TileLayout) {
        HexagonTileMap::update_cells_layout(self.to_gd(), from, to);
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn axis(&self) -> TileOffsetAxis {
        self.cache_key
            .as_ref()
            .map(|key| key.axis)
            .unwrap_or(TileOffsetAxis::HORIZONTAL)
    }
}
