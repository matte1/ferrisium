#!/usr/bin/env sh
set -eu

DEST_DIR="${1:-examples/ferrisium_demo/assets/kernels}"

mkdir -p "$DEST_DIR"

download_kernel() {
    name="$1"
    url="$2"
    output="$DEST_DIR/$name"
    partial="$output.part"

    if [ -s "$output" ]; then
        echo "kernel already present: $output"
        return
    fi

    echo "downloading $name"
    curl -L --fail --show-error --progress-bar --continue-at - --output "$partial" "$url"
    mv "$partial" "$output"
}

download_kernel "de440s.bsp" "http://public-data.nyxspace.com/anise/de440s.bsp"
download_kernel "pck11.pca" "http://public-data.nyxspace.com/anise/v0.7/pck11.pca"
download_kernel "moon_fk_de440.epa" "http://public-data.nyxspace.com/anise/v0.7/moon_fk_de440.epa"
download_kernel "moon_pa_de440_200625.bpc" "http://public-data.nyxspace.com/anise/moon_pa_de440_200625.bpc"

# ANISE's default MetaAlmanac pulls the high-precision Earth orientation file
# from NAIF because it is updated regularly.
download_kernel \
    "earth_latest_high_prec.bpc" \
    "https://naif.jpl.nasa.gov/pub/naif/generic_kernels/pck/earth_latest_high_prec.bpc"
