#!/usr/bin/env python3
"""Converts an equirectangular HDRI panorama (.exr/.hdr) into a Bevy-compatible cubemap skybox,
packaged as a single, Zstandard-supercompressed KTX2 file.

Usage:
    scripts/hdri_to_skybox.py <input.exr> <output.ktx2> [--face-size N] [--zstd-level N]

Requires on PATH / importable:
    - Python 3 with `numpy` and OpenImageIO's Python bindings (`python3 -c "import OpenImageIO"`)
    - the KTX-Software CLI (`ktx`, https://github.com/KhronosGroup/KTX-Software)

How it works: samples 6 square cube faces (order: +X, -X, +Y, -Y, +Z, -Z — the standard
Vulkan/KTX2 cubemap face order) out of the source equirectangular panorama, by taking each face
pixel's 3D view direction and inverse-mapping it back to a longitude/latitude UV on the source
panorama (bilinearly sampled, with horizontal wraparound and vertical clamping at the poles).
Those 6 faces are then hard fed to `ktx create --cubemap`, which assembles and Zstd-compresses
them into one .ktx2 texture in a single step.

Bevy's own KTX2 loader (`bevy_image`) auto-detects a 6-face KTX2 as a cube texture
(`TextureViewDimension::Cube`) directly from the file's metadata, so the result can be loaded and
used as a `Skybox` image with no further reinterpretation needed at runtime — unlike the legacy
PNG-vertical-strip cubemap `client/src/camera.rs` used before (see that file's own comments on
`Image::reinterpret_stacked_2d_as_array`, a workaround needed only because plain PNGs carry no
cubemap metadata at all).
"""

import argparse
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
import OpenImageIO as oiio

# Standard Vulkan/KTX2 cubemap face order — `ktx create --cubemap` expects exactly this order for
# its 6 input images.
FACE_NAMES = ["posx", "negx", "posy", "negy", "posz", "negz"]


def face_directions(face_index: int, size: int) -> np.ndarray:
    """Returns a (size, size, 3) array of unit view directions for one cube face, using the
    standard OpenGL/Vulkan cubemap face basis (row 0 of the returned/rendered image is the top of
    the face, i.e. v = -1)."""
    coords = (np.arange(size, dtype=np.float64) + 0.5) / size * 2.0 - 1.0
    u, v = np.meshgrid(coords, coords)  # u varies along columns, v along rows

    if face_index == 0:  # +X
        dirs = np.stack([np.ones_like(u), -v, -u], axis=-1)
    elif face_index == 1:  # -X
        dirs = np.stack([-np.ones_like(u), -v, u], axis=-1)
    elif face_index == 2:  # +Y
        dirs = np.stack([u, np.ones_like(u), v], axis=-1)
    elif face_index == 3:  # -Y
        dirs = np.stack([u, -np.ones_like(u), -v], axis=-1)
    elif face_index == 4:  # +Z
        dirs = np.stack([u, -v, np.ones_like(u)], axis=-1)
    elif face_index == 5:  # -Z
        dirs = np.stack([-u, -v, -np.ones_like(u)], axis=-1)
    else:
        raise ValueError(face_index)

    norm = np.linalg.norm(dirs, axis=-1, keepdims=True)
    return dirs / norm


def sample_equirect(source: np.ndarray, directions: np.ndarray) -> np.ndarray:
    """Bilinearly samples equirectangular image `source` (H, W, C) at the longitude/latitude
    implied by each unit direction vector in `directions` (size, size, 3) — horizontal wraparound,
    vertical clamping at the poles (there's no "next row" past a pole to wrap to)."""
    src_h, src_w, _channels = source.shape
    x, y, z = directions[..., 0], directions[..., 1], directions[..., 2]

    longitude = np.arctan2(x, -z)  # [-pi, pi]
    latitude = np.arcsin(np.clip(y, -1.0, 1.0))  # [-pi/2, pi/2]

    u = (longitude / (2.0 * np.pi) + 0.5) * src_w
    v = (0.5 - latitude / np.pi) * src_h

    u0 = np.floor(u).astype(np.int64)
    v0 = np.floor(v).astype(np.int64)
    fu = (u - u0)[..., None]
    fv = (v - v0)[..., None]

    u0m = u0 % src_w
    u1m = (u0 + 1) % src_w
    v0c = np.clip(v0, 0, src_h - 1)
    v1c = np.clip(v0 + 1, 0, src_h - 1)

    top = source[v0c, u0m] * (1 - fu) + source[v0c, u1m] * fu
    bottom = source[v1c, u0m] * (1 - fu) + source[v1c, u1m] * fu
    return top * (1 - fv) + bottom * fv


def read_image(path: Path) -> np.ndarray:
    buf = oiio.ImageBuf(str(path))
    if buf.has_error:
        raise RuntimeError(f"failed to read {path}: {buf.geterror()}")
    return buf.get_pixels(oiio.FLOAT)


def write_exr(path: Path, pixels: np.ndarray) -> None:
    height, width, channels = pixels.shape
    spec = oiio.ImageSpec(width, height, channels, oiio.HALF)
    out = oiio.ImageOutput.create(str(path))
    if out is None:
        raise RuntimeError(f"no image writer available for {path}")
    if not out.open(str(path), spec):
        raise RuntimeError(f"failed to open {path} for writing: {out.geterror()}")
    if not out.write_image(pixels.astype(np.float32)):
        raise RuntimeError(f"failed to write {path}: {out.geterror()}")
    out.close()


def main() -> None:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("input", type=Path, help="equirectangular HDRI panorama (.exr/.hdr)")
    parser.add_argument("output", type=Path, help="output .ktx2 cubemap path")
    parser.add_argument(
        "--face-size",
        type=int,
        default=1024,
        help="cube face resolution in pixels, per side (default: 1024)",
    )
    parser.add_argument(
        "--format",
        default="R16G16B16A16_SFLOAT",
        help="ktx create --format (default: R16G16B16A16_SFLOAT, matching a half-float EXR "
        "source losslessly)",
    )
    parser.add_argument(
        "--zstd-level",
        type=int,
        default=19,
        help="ktx create --zstd supercompression level, 1-22 (default: 19)",
    )
    args = parser.parse_args()

    if not args.input.exists():
        sys.exit(f"input file not found: {args.input}")

    print(f"Reading {args.input} ...")
    source = read_image(args.input)
    if source.shape[2] == 3:
        alpha = np.ones((*source.shape[:2], 1), dtype=source.dtype)
        source = np.concatenate([source, alpha], axis=-1)

    args.output.parent.mkdir(parents=True, exist_ok=True)

    with tempfile.TemporaryDirectory(prefix="hdri_to_skybox_") as tmp_dir_str:
        tmp_dir = Path(tmp_dir_str)
        face_paths = []
        for index, name in enumerate(FACE_NAMES):
            print(f"Rendering face {index + 1}/6 ({name}) at {args.face_size}x{args.face_size} ...")
            directions = face_directions(index, args.face_size)
            face_pixels = sample_equirect(source, directions)
            face_path = tmp_dir / f"{name}.exr"
            write_exr(face_path, face_pixels)
            face_paths.append(face_path)

        print(f"Assembling + compressing cubemap into {args.output} with ktx create ...")
        cmd = [
            "ktx",
            "create",
            "--format",
            args.format,
            "--cubemap",
            "--assign-texcoord-origin",
            "top-left",
            "--zstd",
            str(args.zstd_level),
            *[str(p) for p in face_paths],
            str(args.output),
        ]
        subprocess.run(cmd, check=True)

    print(f"Done: {args.output}")


if __name__ == "__main__":
    main()
