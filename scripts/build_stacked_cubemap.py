#!/usr/bin/env python3
"""Project a NASA SVS celestial map into a Bevy stacked cubemap.

The output is a vertical stack of six square faces in Bevy's cubemap layer
order: +X, -X, +Y, -Y, +Z, -Z.

The sky convention baked into this converter is Ferrisium's J2000 convention:
+X is right ascension 0h at declination 0, +Y is right ascension 6h at
declination 0, and +Z is the north celestial pole. NASA's celestial Deep Star
Maps use plate-carree ICRF/J2000 right ascension and declination, centered at
0h with right ascension increasing toward image-left.

Bevy's skybox shader samples cubemaps with Z negated. This converter accounts
for that so an identity `GlobeSkybox` rotation corresponds to Ferrisium's J2000
frame.
"""

from __future__ import annotations

import argparse
import math
from pathlib import Path

import numpy as np
from PIL import Image

Image.MAX_IMAGE_PIXELS = None

FACE_ORDER = ("+X", "-X", "+Y", "-Y", "+Z", "-Z")

COLOR_RA_0 = np.array([230, 30, 30], dtype=np.uint8)
COLOR_RA_6 = np.array([30, 220, 80], dtype=np.uint8)
COLOR_RA_12 = np.array([50, 110, 240], dtype=np.uint8)
COLOR_NORTH_POLE = np.array([250, 230, 40], dtype=np.uint8)
COLOR_SOUTH_POLE = np.array([230, 40, 230], dtype=np.uint8)


def face_direction(face: str, a: np.ndarray, b: np.ndarray):
    """Return unnormalized Bevy cubemap sample directions for one face."""
    if face == "+X":
        return 1.0, -b, -a
    if face == "-X":
        return -1.0, -b, a
    if face == "+Y":
        return a, 1.0, b
    if face == "-Y":
        return a, -1.0, -b
    if face == "+Z":
        return a, -b, 1.0
    if face == "-Z":
        return -a, -b, -1.0
    raise ValueError(f"unknown cubemap face {face}")


def j2000_from_bevy_sample(
    cube_x: np.ndarray | float,
    cube_y: np.ndarray | float,
    cube_z: np.ndarray | float,
):
    """Invert Bevy's skybox cubemap Z flip."""
    return cube_x, cube_y, -cube_z


def sample_equirectangular(src: np.ndarray, x: np.ndarray, y: np.ndarray, z: np.ndarray):
    """Bilinearly sample an RGB NASA celestial equirectangular image."""
    height, width, _ = src.shape
    inv_len = 1.0 / np.sqrt(x * x + y * y + z * z, dtype=np.float32)
    x = x * inv_len
    y = y * inv_len
    z = z * inv_len

    right_ascension = np.arctan2(y, x) % (2.0 * math.pi)
    declination = np.arcsin(np.clip(z, -1.0, 1.0))

    # NASA's celestial maps are centered at 0h RA and RA increases leftward.
    u = (0.5 - right_ascension / (2.0 * math.pi)) % 1.0
    v = 0.5 - declination / math.pi

    px = u * width
    py = np.clip(v * (height - 1), 0.0, height - 1.0)

    x0 = np.floor(px).astype(np.int64) % width
    y0 = np.floor(py).astype(np.int64)
    x1 = (x0 + 1) % width
    y1 = np.minimum(y0 + 1, height - 1)

    dx = (px - np.floor(px))[..., None].astype(np.float32)
    dy = (py - np.floor(py))[..., None].astype(np.float32)

    p00 = src[y0, x0].astype(np.float32)
    p10 = src[y0, x1].astype(np.float32)
    p01 = src[y1, x0].astype(np.float32)
    p11 = src[y1, x1].astype(np.float32)

    top = p00 * (1.0 - dx) + p10 * dx
    bottom = p01 * (1.0 - dx) + p11 * dx
    return np.clip(top * (1.0 - dy) + bottom * dy, 0.0, 255.0).astype(np.uint8)


def build_face(src: np.ndarray, face: str, face_size: int, chunk_rows: int) -> Image.Image:
    """Build a single cubemap face in row chunks to keep memory bounded."""
    columns = np.arange(face_size, dtype=np.float32)
    a = (2.0 * (columns + 0.5) / face_size - 1.0)[None, :]
    face_pixels = np.empty((face_size, face_size, 3), dtype=np.uint8)

    for y0 in range(0, face_size, chunk_rows):
        y1 = min(y0 + chunk_rows, face_size)
        rows = np.arange(y0, y1, dtype=np.float32)
        b = (2.0 * (rows + 0.5) / face_size - 1.0)[:, None]
        cube_x, cube_y, cube_z = face_direction(face, a, b)
        x, y, z = j2000_from_bevy_sample(cube_x, cube_y, cube_z)
        face_pixels[y0:y1, :, :] = sample_equirectangular(src, x, y, z)

    return Image.fromarray(face_pixels, mode="RGB")


def synthetic_source(width: int = 1024, height: int = 512) -> np.ndarray:
    """Build a marker map for validating face centers and Bevy's Z flip."""
    pixels = np.zeros((height, width, 3), dtype=np.uint8)
    for row in range(height):
        v = (row + 0.5) / height
        declination = 90.0 - 180.0 * v
        for col in range(width):
            u = (col + 0.5) / width
            right_ascension = ((0.5 - u) % 1.0) * 24.0
            if declination >= 75.0:
                pixels[row, col] = COLOR_NORTH_POLE
            elif declination <= -75.0:
                pixels[row, col] = COLOR_SOUTH_POLE
            elif right_ascension <= 1.0 or right_ascension >= 23.0:
                pixels[row, col] = COLOR_RA_0
            elif 5.0 <= right_ascension <= 7.0:
                pixels[row, col] = COLOR_RA_6
            elif 11.0 <= right_ascension <= 13.0:
                pixels[row, col] = COLOR_RA_12
    return pixels


def assert_center_color(src: np.ndarray, face: str, expected: np.ndarray) -> None:
    face_size = 17
    image = build_face(src, face, face_size, face_size)
    center = np.asarray(image)[face_size // 2, face_size // 2]
    if not np.array_equal(center, expected):
        raise AssertionError(
            f"{face} center expected {expected.tolist()} but got {center.tolist()}"
        )


def run_self_test() -> None:
    src = synthetic_source()
    assert_center_color(src, "+X", COLOR_RA_0)
    assert_center_color(src, "+Y", COLOR_RA_6)
    assert_center_color(src, "-X", COLOR_RA_12)
    assert_center_color(src, "-Z", COLOR_NORTH_POLE)
    assert_center_color(src, "+Z", COLOR_SOUTH_POLE)
    print("cubemap conversion self-test passed", flush=True)


def build_stacked_cubemap(source_path: Path, output_path: Path, face_size: int, chunk_rows: int):
    source = Image.open(source_path).convert("RGB")
    src = np.asarray(source)
    print(f"source {source.width}x{source.height}", flush=True)

    stacked = Image.new("RGB", (face_size, face_size * len(FACE_ORDER)))
    for index, face in enumerate(FACE_ORDER):
        print(f"building face {face}", flush=True)
        image = build_face(src, face, face_size, chunk_rows)
        stacked.paste(image, (0, index * face_size))

    output_path.parent.mkdir(parents=True, exist_ok=True)
    stacked.save(output_path, compress_level=6)
    print(f"wrote {output_path}", flush=True)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("source", nargs="?", type=Path)
    parser.add_argument("output", nargs="?", type=Path)
    parser.add_argument("--face-size", type=int, default=4096)
    parser.add_argument("--chunk-rows", type=int, default=128)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        run_self_test()
        return

    if args.source is None or args.output is None:
        parser.error("source and output are required unless --self-test is used")

    build_stacked_cubemap(args.source, args.output, args.face_size, args.chunk_rows)


if __name__ == "__main__":
    main()
