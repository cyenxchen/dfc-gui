#!/usr/bin/env python3

import logging
import math
import shutil
import subprocess
import tempfile
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont


ROOT = Path(__file__).resolve().parent.parent
ASSETS_DIR = ROOT / "assets"
ICONS_DIR = ROOT / "icons"
PNG_PATH = ASSETS_DIR / "icon.png"
ICO_PATH = ASSETS_DIR / "icon.ico"
ICNS_PATH = ICONS_DIR / "dfc-gui.icns"
FONT_PATH = Path("/System/Library/Fonts/Supplemental/Arial Bold.ttf")
ICON_SIZE = 1024


def configure_logging() -> None:
    logging.basicConfig(level=logging.INFO, format="[icon-gen] %(message)s")


def build_vertical_gradient(size: int, top: tuple[int, int, int], bottom: tuple[int, int, int]) -> Image.Image:
    gradient = Image.new("RGBA", (1, size))
    pixels = []
    for y in range(size):
        t = y / max(size - 1, 1)
        pixels.append(
            (
                round(top[0] + (bottom[0] - top[0]) * t),
                round(top[1] + (bottom[1] - top[1]) * t),
                round(top[2] + (bottom[2] - top[2]) * t),
                255,
            )
        )
    gradient.putdata(pixels)
    return gradient.resize((size, size))


def build_radial_highlight(size: int, center: tuple[float, float], radius: float, color: tuple[int, int, int]) -> Image.Image:
    layer = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    pixels = layer.load()
    cx, cy = center
    for y in range(size):
        for x in range(size):
            distance = math.hypot(x - cx, y - cy)
            strength = max(0.0, 1.0 - (distance / radius))
            if strength <= 0:
                continue
            alpha = round((strength ** 1.85) * 140)
            pixels[x, y] = (*color, alpha)
    return layer


def rounded_rect_mask(size: int, inset: int, radius: int) -> Image.Image:
    mask = Image.new("L", (size, size), 0)
    draw = ImageDraw.Draw(mask)
    draw.rounded_rectangle(
        (inset, inset, size - inset, size - inset),
        radius=radius,
        fill=255,
    )
    return mask


def draw_icon() -> Image.Image:
    logging.info("Rendering %sx%s icon source", ICON_SIZE, ICON_SIZE)
    canvas = Image.new("RGBA", (ICON_SIZE, ICON_SIZE), (0, 0, 0, 0))

    shadow_mask = rounded_rect_mask(ICON_SIZE, inset=116, radius=212)
    shadow = Image.new("RGBA", (ICON_SIZE, ICON_SIZE), (0, 0, 0, 0))
    shadow.paste((8, 12, 20, 185), mask=shadow_mask)
    shadow = shadow.filter(ImageFilter.GaussianBlur(38))
    canvas.alpha_composite(shadow, dest=(0, 36))

    base_mask = rounded_rect_mask(ICON_SIZE, inset=88, radius=196)
    base = build_vertical_gradient(ICON_SIZE, (58, 70, 92), (20, 27, 39))

    cool_glow = Image.new("RGBA", (ICON_SIZE, ICON_SIZE), (0, 0, 0, 0))
    cool_glow = Image.alpha_composite(
        cool_glow,
        build_radial_highlight(ICON_SIZE, (300, 230), 420, (96, 138, 198)),
    )
    cool_glow = Image.alpha_composite(
        cool_glow,
        build_radial_highlight(ICON_SIZE, (760, 760), 320, (70, 112, 182)),
    )
    base = Image.alpha_composite(base, cool_glow)

    top_sheen = Image.new("RGBA", (ICON_SIZE, ICON_SIZE), (0, 0, 0, 0))
    sheen_draw = ImageDraw.Draw(top_sheen)
    sheen_draw.pieslice(
        (-40, -280, 1100, 780),
        start=18,
        end=150,
        fill=(255, 255, 255, 34),
    )
    sheen_draw.rounded_rectangle(
        (150, 130, 874, 330),
        radius=130,
        fill=(255, 255, 255, 20),
    )
    top_sheen = top_sheen.filter(ImageFilter.GaussianBlur(44))
    base = Image.alpha_composite(base, top_sheen)

    lower_accent = Image.new("RGBA", (ICON_SIZE, ICON_SIZE), (0, 0, 0, 0))
    accent_draw = ImageDraw.Draw(lower_accent)
    accent_draw.pieslice(
        (420, 340, 1180, 1140),
        start=190,
        end=336,
        fill=(98, 148, 225, 50),
    )
    lower_accent = lower_accent.filter(ImageFilter.GaussianBlur(40))
    base = Image.alpha_composite(base, lower_accent)

    border = Image.new("RGBA", (ICON_SIZE, ICON_SIZE), (0, 0, 0, 0))
    border_draw = ImageDraw.Draw(border)
    border_draw.rounded_rectangle(
        (88, 88, 936, 936),
        radius=196,
        outline=(214, 225, 241, 115),
        width=10,
    )
    border_draw.rounded_rectangle(
        (102, 102, 922, 922),
        radius=182,
        outline=(8, 12, 18, 122),
        width=6,
    )
    base = Image.alpha_composite(base, border)

    base_mask_rgba = Image.new("RGBA", (ICON_SIZE, ICON_SIZE), (0, 0, 0, 0))
    base_mask_rgba.paste(base, mask=base_mask)
    canvas.alpha_composite(base_mask_rgba)

    font = ImageFont.truetype(str(FONT_PATH), 172)
    text = "DFC-GUI"
    draw = ImageDraw.Draw(canvas)

    bbox = draw.textbbox((0, 0), text, font=font, stroke_width=6)
    text_width = bbox[2] - bbox[0]
    text_height = bbox[3] - bbox[1]
    x = (ICON_SIZE - text_width) / 2
    y = (ICON_SIZE - text_height) / 2 - 8

    draw.text(
        (x, y + 10),
        text,
        font=font,
        fill=(0, 0, 0, 110),
        stroke_width=8,
        stroke_fill=(0, 0, 0, 60),
    )
    draw.text(
        (x, y),
        text,
        font=font,
        fill=(242, 246, 252, 255),
        stroke_width=6,
        stroke_fill=(18, 24, 34, 120),
    )

    text_glow = Image.new("RGBA", (ICON_SIZE, ICON_SIZE), (0, 0, 0, 0))
    glow_draw = ImageDraw.Draw(text_glow)
    glow_draw.text(
        (x, y - 2),
        text,
        font=font,
        fill=(255, 255, 255, 82),
        stroke_width=0,
    )
    text_glow = text_glow.filter(ImageFilter.GaussianBlur(18))
    canvas.alpha_composite(text_glow)
    draw = ImageDraw.Draw(canvas)
    draw.text(
        (x, y),
        text,
        font=font,
        fill=(242, 246, 252, 255),
        stroke_width=6,
        stroke_fill=(18, 24, 34, 120),
    )

    return canvas


def save_png_and_ico(image: Image.Image) -> None:
    logging.info("Saving PNG to %s", PNG_PATH)
    ASSETS_DIR.mkdir(parents=True, exist_ok=True)
    image.save(PNG_PATH, format="PNG")

    logging.info("Saving ICO to %s", ICO_PATH)
    image.save(
        ICO_PATH,
        format="ICO",
        sizes=[(256, 256), (128, 128), (64, 64), (48, 48), (32, 32), (16, 16)],
    )


def build_icns(image: Image.Image) -> None:
    logging.info("Building ICNS at %s", ICNS_PATH)
    ICONS_DIR.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as temp_dir:
        iconset_dir = Path(temp_dir) / "dfc-gui.iconset"
        iconset_dir.mkdir()
        icon_sizes = [
            (16, "icon_16x16.png"),
            (32, "icon_16x16@2x.png"),
            (32, "icon_32x32.png"),
            (64, "icon_32x32@2x.png"),
            (128, "icon_128x128.png"),
            (256, "icon_128x128@2x.png"),
            (256, "icon_256x256.png"),
            (512, "icon_256x256@2x.png"),
            (512, "icon_512x512.png"),
            (1024, "icon_512x512@2x.png"),
        ]
        for size, filename in icon_sizes:
            resized = image.resize((size, size), Image.Resampling.LANCZOS)
            resized.save(iconset_dir / filename, format="PNG")

        subprocess.run(
            ["iconutil", "-c", "icns", str(iconset_dir), "-o", str(ICNS_PATH)],
            check=True,
        )


def main() -> None:
    configure_logging()
    if not FONT_PATH.exists():
        raise FileNotFoundError(f"Missing required font: {FONT_PATH}")

    image = draw_icon()
    save_png_and_ico(image)
    build_icns(image)
    logging.info("Icon generation complete")


if __name__ == "__main__":
    main()
