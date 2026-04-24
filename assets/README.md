# Ferrisium Assets

This directory contains optional default assets that applications can copy into
their Bevy asset folder.

Asset provenance is recorded in
[THIRD_PARTY_ASSETS.md](THIRD_PARTY_ASSETS.md).

For Trunk apps, copy the bundled Ferrisium asset tree into the browser build
with an `index.html` entry like:

```html
<link
  data-trunk
  rel="copy-dir"
  href="../../assets/ferrisium"
  data-target-path="assets/ferrisium"
/>
```

If an app only uses one resolution, it can copy that single PNG to the path
returned by `MilkyWaySkyboxResolution::asset_path()`.

Then load a bundled Milky Way skybox with:

```rust
app.insert_resource(
    GlobeSkybox::milky_way(MilkyWaySkyboxResolution::Face1024).deferred(),
);
```

The files are vertically stacked PNG cubemaps in Bevy's six-face order. The
resolution name is the square face size; the PNG height is six times that size.

| Variant | Asset path | PNG dimensions | Approx decoded RGBA |
| --- | --- | --- | --- |
| `Face512` | `ferrisium/skyboxes/milkyway_512.png` | 512 x 3072 | 6 MiB |
| `Face1024` | `ferrisium/skyboxes/milkyway_1024.png` | 1024 x 6144 | 24 MiB |
| `Face2048` | `ferrisium/skyboxes/milkyway_2048.png` | 2048 x 12288 | 96 MiB |
| `Face4096` | `ferrisium/skyboxes/milkyway_4096.png` | 4096 x 24576 | 384 MiB |

The full-resolution file is intentionally large and should be loaded with
`DeferredGlobeSkybox` in browser demos.
