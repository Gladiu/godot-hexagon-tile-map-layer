use godot::prelude::*;

mod hexagon_tile_map;
mod hexagon_tile_map_layer;

/// GDExtension entry point for the `HexagonTileMapLayer` addon.
///
/// The two public Godot classes are:
/// - `HexagonTileMap` (see [`hexagon_tile_map`])
/// - `HexagonTileMapLayer` (see [`hexagon_tile_map_layer`])
struct HexagonTileMapExtension;

#[gdextension]
unsafe impl ExtensionLibrary for HexagonTileMapExtension {}
