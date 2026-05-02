# Ferrisium

Ferrisium is an early browser-first geospatial scene layer for Rust and Bevy.
It renders 2D raster maps, focused 3D globes, geodetic drawing
layers, and metric space scenes in WebGL2 wasm builds.

This repo is a prototype. The demo is useful, but the public API is still
moving and there is no release tag yet.

## What Works

- browser-first Bevy map and globe rendering through `FerrisiumPlugin`
- Earth Web Mercator raster sources, NASA GIBS Blue Marble, Mapbox raster
  helpers, and NASA Trek equirectangular regular-body sources
- focused 3D globe controls with mouse, wheel, touch, and surface-grab panning
- H3 map/globe overlays with fills, outlines, per-cell colors, hover state, and
  click messages
- geodetic polylines and convex no-hole geodetic polygons on the map and globe
- globe-space placement through `GlobePosition`, `GlobeLink`, and `GlobeLabel`
- metric scene objects, visual-radius policies, trajectories, orbit camera
  controls, a NASA GLB Sun model, Earth night-lights for globe/solar-system
  views, and shader-backed cloud/atmosphere layers for solar-system style views
- ANISE ephemeris provider adapter with deterministic demo fallback data

## Not Yet

- stable v0.1 API

## Requirements

- Rust 1.91 or newer
- `wasm32-unknown-unknown`
- `just`
- `trunk`
- Node.js/npm
- Chrome or Chromium for browser inspection and Playwright tests

Install the wasm target if needed:

```bash
rustup target add wasm32-unknown-unknown
```

## Run The Demo

Start the browser demo:

```bash
just web
```

Open:

```text
http://127.0.0.1:8081
```

The demo is browser-only. Native `cargo run` intentionally exits with an error
so the wasm/browser path stays the validated workflow.

Useful demo URLs and parameters:

| Goal                        | URL or parameter                                                                                                                                                        |
| --------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| default close globe         | `http://127.0.0.1:8081`                                                                                                                                                 |
| 2D map                      | `?view=map`                                                                                                                                                             |
| metric solar-system scene   | `?view=solar`                                                                                                                                                           |
| close globe focus           | `?focus=earth`, `mercury`, `venus`, `moon`, `mars`, `ceres`, `io`, `europa`, `ganymede`, `callisto`, `titan`, `dione`, `enceladus`, `iapetus`, `mimas`, `rhea`, or `tethys` |
| map body                    | `?view=map&map_body=earth`, `mercury`, `venus`, `moon`, `mars`, `ceres`, `io`, `europa`, `ganymede`, `callisto`, `titan`, `dione`, `enceladus`, `iapetus`, `mimas`, `rhea`, or `tethys` |
| solar focus                 | `?view=solar&solar_focus=scene`, `sun`, `mercury`, `venus`, `earth`, `moon`, or `mars`                                                                                   |
| solar epoch offset          | `?view=solar&solar_days=30` offsets the solar-system scene from J2000 by days                                                                                            |
| solar time playback         | `?view=solar&solar_running=1&solar_speed=7` starts solar time at the selected days-per-browser-second speed                                                              |
| Earth tile source           | `?tile_source=nasa-blue-marble`, `openstreetmap`, `mapbox-satellite`, `mapbox-streets`, `mapbox-outdoors`, `mapbox-light`, `mapbox-dark`, or `mapbox-satellite-streets` |
| Mapbox public token         | `?mapbox_token=pk...`                                                                                                                                                   |
| H3 globe inspection overlay | `?h3_inspect=1`                                                                                                                                                         |
| egui input-capture example  | `?egui=1`                                                                                                                                                               |
| skip staged ANISE kernels   | `?no_anise=1`                                                                                                                                                           |
| inspection camera target    | `?globe_lon=-96&globe_lat=39&globe_distance_factor=1.5`                                                                                                                 |

For a one-shot build and no-cache local server:

```bash
just web-once
```

For an optimized wasm build:

```bash
just web-release-once
```

## Minimal Examples

Run the focused examples with the repo-level recipes:

```bash
just web-map
```

```bash
just web-globe
```

`just web-demo` is an explicit alias for the full demo; `just web` remains the
short default.

Release builds use the same names with `-release`:

```bash
just web-map-release
just web-globe-release
```

## Development

List recipes:

```bash
just --list
```

Common checks:

```bash
just fmt-check
just lint
just lint-wasm
just check-wasm
just test
just doc
```

Run the full local quality gate:

```bash
just quality
```

Remove generated build artifacts and locally staged demo kernels:

```bash
just clean
```

Browser smoke tests:

```bash
npm install
npx playwright install ffmpeg
just web-test
```

Manual browser inspection:

```bash
just web-inspect
just web-inspect -- --scenario h3-globe-inspect
just web-inspect -- --path '/?view=map' --scenario map-wheel
just web-inspect -- --path '/?view=solar' --scenario solar-wheel
```

## Assets And Data

Optional default assets live under [assets/](assets/). Copy
`assets/ferrisium` into an app's Bevy asset folder to use the bundled skyboxes.
Trunk apps can copy the tree at build time:

```html
<link
  data-trunk
  rel="copy-dir"
  href="../../assets/ferrisium"
  data-target-path="assets/ferrisium"
/>
```

Apps that only use one resolution can copy that single PNG to the path returned
by `MilkyWaySkyboxResolution::asset_path()`.

Then configure a single built-in Milky Way skybox:

```rust
app.insert_resource(
    GlobeSkybox::milky_way(MilkyWaySkyboxResolution::Face1024).deferred(),
);
```

For browser apps that should show stars early and sharpen the background later,
use the progressive skybox resource:

```rust
app.insert_resource(
    ProgressiveGlobeSkybox::milky_way([
        MilkyWaySkyboxResolution::Face512,
        MilkyWaySkyboxResolution::Face1024,
        MilkyWaySkyboxResolution::Face2048,
    ]),
);
```

Available Milky Way cubemap variants use 512, 1024, 2048, and 4096 pixel faces.
The 4096 variant is full resolution and should be deferred in browser views.
The bundled demo starts with the 512px variant and progressively upgrades
through 1024px and 2048px variants so the background appears early without
automatically fetching the full-resolution skybox.

The demo defaults to public no-key sources where possible:

- NASA GIBS Blue Marble for the close globe
- OpenStreetMap for the Earth map
- NASA Trek regular-body sources for Mercury, Venus, Moon, Mars, Ceres, the
  Galilean moons, and supported regular Saturnian moons

Irregular Trek targets such as Phobos, Phoebe, Vesta, Bennu, and Ryugu are not
exposed yet because they need shape or mesh support rather than spherical
equirectangular body rendering.

Mapbox sources require a public `pk...` token. Do not embed secret Mapbox
tokens in browser wasm.

The browser demo may load ANISE kernels from `examples/ferrisium_demo/assets/kernels/`
when they are staged locally:

```bash
just kernels
```

Those kernel files are intentionally ignored by Git. The demo falls back to a
deterministic built-in model when kernels are absent.

Bundled third-party demo asset provenance is recorded in
[examples/ferrisium_demo/assets/THIRD_PARTY_ASSETS.md](examples/ferrisium_demo/assets/THIRD_PARTY_ASSETS.md).
Default asset provenance is recorded in
[assets/THIRD_PARTY_ASSETS.md](assets/THIRD_PARTY_ASSETS.md).

## Workspace

- `crates/ferrisium_core`: renderer-neutral coordinates, projections, raster
  lifecycle helpers, globe tile selection, celestial spatial types, and
  trajectories
- `crates/ferrisium_bevy`: Bevy plugin, map/globe rendering, browser fetch and
  decode, input, H3/geodetic layers, metric visuals, and skyboxes
- `crates/ferrisium_anise`: ANISE adapter for Ferrisium's ephemeris provider
  trait
- `examples/ferrisium_demo`: full browser demo
- `examples/minimal_map`, `examples/minimal_globe`: focused starter examples

## License

Ferrisium source code is licensed under the MIT License; see [LICENSE](LICENSE).
