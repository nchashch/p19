# Asset credits

Third-party assets used by the game. All of them are dedicated to the public domain under
[CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/), which doesn't require
attribution; they're credited here with thanks to their authors.

| Asset | Author | Source | License | Used in |
|---|---|---|---|---|
| Input Prompts (keyboard & mouse and Steam Deck glyph sheets) | Kenney | [kenney.nl/assets/input-prompts](https://kenney.nl/assets/input-prompts) | CC0 1.0 | `client/textures/input_prompts/` |
| Night Sky HDRI 012 | ambientCG | [ambientcg.com/view?id=NightSkyHDRI012](https://ambientcg.com/view?id=NightSkyHDRI012) | CC0 1.0 | `client/skyboxes/night_sky.ktx2` (converted to a cubemap) |
| Rubber Tiles | Amal Kumar, Poly Haven | [polyhaven.com/a/rubber_tiles](https://polyhaven.com/a/rubber_tiles) | CC0 1.0 | floor texture in `client/rigs/environment/start.glb` and `server/rigs/environment/start.glb` |

The game's Credits screen shows the same list (`crates/client/src/ui/credits.rs`'s `CREDITS`);
keep the two in sync.

Everything else under `assets/` is original work for this project (see the repository README's
"License" section).
