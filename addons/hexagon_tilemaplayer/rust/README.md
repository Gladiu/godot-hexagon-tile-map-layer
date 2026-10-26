# Rust GDExtension version

This directory contains a [godot-rust](https://godot-rust.github.io) port of the two
GDScript classes from the addon:

| GDScript source | Rust class | File |
| --- | --- | --- |
| `hexagon_tilemap.gd` | `HexagonTileMap` | `src/hexagon_tile_map.rs` |
| `hexagon_tilemaplayer.gd` | `HexagonTileMapLayer` | `src/hexagon_tile_map_layer.rs` |

`TileShape`, which was a nested class in GDScript, is exposed as the top-level
`HexagonTileShape` class in `src/hexagon_tile_map_layer.rs` (GDExtension has no
nested classes).

## Build

```bash
cd addons/hexagon_tilemaplayer/rust
cargo build
```

The addon directory already contains `hexagon_tilemaplayer.gdextension`, which
points Godot at the produced library.

## Notes

- Requires Godot 4.4+ (the project targets 4.7).
- The GDScript `plugin.gd` still registers the GDScript `HexagonTileMapLayer`
  custom type. When switching to the Rust classes, update or disable that
  plugin registration to avoid a `ClassDB` name conflict.
- `DebugModeFlags` is not exposed as a nested Godot enum because GDExtension
  does not support nested enums. Use `1` (`TILES_COORDS`) and `2`
  (`CONNECTIONS`) for the `debug_mode` integer flags.
- `cube_explore`'s `maybe` parameter has no default value in this port because
  `Callable` is not an allowed immutable `#[opt]` default type.
