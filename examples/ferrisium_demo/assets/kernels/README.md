# ANISE Kernel Assets

The demo loads ANISE kernels from this directory at runtime, but the kernel
files themselves are intentionally not committed to the repository. They are
large third-party data files and should be staged locally or by deployment
automation with:

```bash
just kernels
```

The download recipe mirrors ANISE's default `MetaAlmanac` bundle where possible:

- `de440s.bsp`
- `pck11.pca`
- `moon_fk_de440.epa`
- `moon_pa_de440_200625.bpc`
- `earth_latest_high_prec.bpc`

The first four files come from the ANISE/Nyx public bucket. The Earth
high-precision orientation file comes from NAIF, matching ANISE's default
bundle because that file is updated regularly.
