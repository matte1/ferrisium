# Third-Party Default Assets

The Ferrisium source code is licensed separately by the repository root
`LICENSE`. This file records the provenance of binary assets bundled under
`assets/`.

NASA's media usage guidelines state that NASA images, media, and 3D texture
maps generally are not subject to U.S. copyright, but NASA should be
acknowledged as the source and NASA marks or imagery must not be used to imply
endorsement. See:

- <https://www.nasa.gov/nasa-brand-center/images-and-media/>

## ferrisium/skyboxes/milkyway_*.png

- Source page: <https://svs.gsfc.nasa.gov/4851/>
- Source file:
  <https://svs.gsfc.nasa.gov/vis/a000000/a004800/a004851/starmap_2020_16k.exr>
- Upstream title: "Deep Star Maps 2020".
- Upstream description: celestial-coordinate plate carrée star map generated
  from Hipparcos-2, Tycho-2, Gaia Data Release 2, Yale Bright Star Catalog,
  UCAC3, and XHIP catalog data.
- Credit: NASA's Scientific Visualization Studio / Ernie Wright.
- Local processing: converted from the NASA 16k OpenEXR equirectangular map
  into an 8-bit sRGB vertically stacked cubemap for Bevy skybox loading. The
  six layers are packed in standard cubemap order: +X, -X, +Y, -Y, +Z, -Z. The
  conversion uses Ferrisium's inertial J2000 sky convention: +X is right
  ascension 0h at declination 0, +Y is right ascension 6h at declination 0, and
  +Z is the north celestial pole. Bevy's skybox cubemap Z-flip is baked into
  the conversion so identity `GlobeSkybox` rotation is J2000-aligned. This is
  not a local-observer horizon or sidereal-time model.
- Downsampled variants: generated from `milkyway_4096.png` with Netpbm
  (`pngtopnm`, `pnmscale`, `pnmtopng`).
- Source EXR SHA-256:
  `19a1351f00c386a6e5eec4d67af96d5fc71edf6a1189941579b9498b52e7589a`.
- Local PNG SHA-256:
  - `milkyway_512.png`:
    `255e57911ae4c571616d785c194dd1b6bd937fb60aa4f8b7968ebda11aa6b921`
  - `milkyway_1024.png`:
    `4d796d30618049abee6de322c915338f56cf476d12c131f41afdf43f9c36d478`
  - `milkyway_2048.png`:
    `bd2c0b12eed4971db5c39eb62f87e3f7ff2da865fe79db6f88a027f0e909dcc5`
  - `milkyway_4096.png`:
    `51bbf938960667c12043514cc2f6d1107fd0a96609e54b2944dd20301daf065b`
