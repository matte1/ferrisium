# Third-Party Demo Assets

The Ferrisium source code is licensed separately by the repository root
`LICENSE`. This file records the provenance of binary demo assets bundled under
`examples/ferrisium_demo/assets/`.

The default Ferrisium skybox assets used by the demo are bundled under the
repository root `assets/` directory and documented in
[`../../../assets/THIRD_PARTY_ASSETS.md`](../../../assets/THIRD_PARTY_ASSETS.md).

NASA's media usage guidelines state that NASA images, media, and 3D texture
maps generally are not subject to U.S. copyright, but NASA should be
acknowledged as the source and NASA marks or imagery must not be used to imply
endorsement. See:

- <https://www.nasa.gov/nasa-brand-center/images-and-media/>

## textures/venus.jpg

- Source page: <https://science.nasa.gov/3d-resources/venus/>
- Source file:
  <https://assets.science.nasa.gov/content/dam/science/cds/3d/resources/image/venus/Venus.jpg>
- Upstream description: image texture for 3D models, stitched from Magellan
  RADAR imagery with gaps filled from a global texture; from the JPL/Caltech
  generated planetary maps database.
- Credit: NASA/JPL-Caltech.
- Local SHA-256:
  `e395bced59e95339efbdd002d4a0bbbf81fe323a6694f3a4039120560c80e789`.

## textures/earth_clouds_2048.jpg

- Source page: <https://visibleearth.nasa.gov/images/57747/blue-marble-clouds>
- Source file:
  <https://eoimages.gsfc.nasa.gov/images/imagerecords/57000/57747/cloud_combined_2048.jpg>
- Upstream description: Blue Marble cloud composite from MODIS visible-light
  imagery and thermal infrared imagery over the poles.
- Credit: NASA Goddard Space Flight Center; image by Reto Stockli, enhancements
  by Robert Simmon; MODIS data and technical support from NASA teams listed on
  the source page.
- Local SHA-256:
  `daddaad84d7a33bbbc86cdda3f591099f57cee8607b7bcf3b67eb7e4f7a1c793`.

## textures/earth_night_lights_2048.jpg

- Source page: <https://science.nasa.gov/earth/earth-observatory/earth-at-night/maps/>
- Source file:
  <https://assets.science.nasa.gov/content/dam/science/esd/eo/images/imagerecords/144000/144897/BlackMarble_2016_01deg_gray.jpg>
- Upstream description: Black Marble 2016 grayscale map of Earth lights at
  night from Suomi NPP VIIRS observations.
- Local processing: downsampled to 2048x1024 with ImageMagick `convert`.
- Credit: NASA Earth Observatory images by Joshua Stevens, using Suomi NPP
  VIIRS data from Miguel Roman, NASA GSFC.
- Local SHA-256:
  `273879fffa39c66edb97ae3ce29d9808714d3236dc068b1bea5597d8910c247c`.

## models/nasa_sun.glb

- Source page: <https://solarsystem.nasa.gov/gltf_embed/2352/>
- Current NASA Science resource page:
  <https://science.nasa.gov/learn/heat/resource/sun-3d-model/>
- Source file:
  <https://solarsystem.nasa.gov/rails/active_storage/blobs/redirect/eyJfcmFpbHMiOnsibWVzc2FnZSI6IkJBaHBBblVRIiwiZXhwIjpudWxsLCJwdXIiOiJibG9iX2lkIn19--abda6331ea1271cb16bf7b8b08f42b0ad49115b2/Sun_1_1391000.glb?disposition=inline>
- Upstream description: 3D model of the Sun, our star.
- Credit: NASA.
- Local SHA-256:
  `a178f5c43c2c9a6ee6c011c315149a7ab5a2594fd5aa9119ee219625a18da1dd`.
