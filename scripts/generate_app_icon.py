#!/usr/bin/env python3

import logging
import math
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


def composite_masked(
    canvas: Image.Image, layer: Image.Image, mask: Image.Image, dest: tuple[int, int] = (0, 0)
) -> None:
    clipped = Image.new("RGBA", (ICON_SIZE, ICON_SIZE), (0, 0, 0, 0))
    clipped.paste(layer, mask=mask)
    canvas.alpha_composite(clipped, dest=dest)


def draw_icon() -> Image.Image:
    logging.info("Rendering %sx%s icon source without the outer white shell", ICON_SIZE, ICON_SIZE)
    canvas = Image.new("RGBA", (ICON_SIZE, ICON_SIZE), (0, 0, 0, 0))

    panel_inset = 112
    panel_radius = 206
    panel_bounds = (panel_inset, panel_inset, ICON_SIZE - panel_inset, ICON_SIZE - panel_inset)
    panel_mask = rounded_rect_mask(ICON_SIZE, inset=panel_inset, radius=panel_radius)

    panel_shadow = Image.new("RGBA", (ICON_SIZE, ICON_SIZE), (0, 0, 0, 0))
    panel_shadow.paste((7, 10, 18, 168), mask=panel_mask)
    panel_shadow = panel_shadow.filter(ImageFilter.GaussianBlur(42))
    canvas.alpha_composite(panel_shadow, dest=(0, 30))

    panel = build_vertical_gradient(ICON_SIZE, (79, 98, 130), (21, 31, 51))
    panel = Image.alpha_composite(
        panel,
        build_radial_highlight(ICON_SIZE, (330, 308), 320, (126, 160, 220)),
    )
    panel = Image.alpha_composite(
        panel,
        build_radial_highlight(ICON_SIZE, (778, 720), 360, (83, 123, 193)),
    )
    panel = Image.alpha_composite(
        panel,
        build_radial_highlight(ICON_SIZE, (760, 394), 330, (188, 202, 224)),
    )

    panel_gloss = Image.new("RGBA", (ICON_SIZE, ICON_SIZE), (0, 0, 0, 0))
    panel_gloss_draw = ImageDraw.Draw(panel_gloss)
    panel_gloss_draw.pieslice(
        (20, -70, 1150, 850),
        start=20,
        end=136,
        fill=(255, 255, 255, 38),
    )
    panel_gloss_draw.ellipse((420, 110, 980, 548), fill=(255, 255, 255, 20))
    panel_gloss = panel_gloss.filter(ImageFilter.GaussianBlur(52))
    panel = Image.alpha_composite(panel, panel_gloss)

    panel_vignette = Image.new("RGBA", (ICON_SIZE, ICON_SIZE), (0, 0, 0, 0))
    panel_vignette_draw = ImageDraw.Draw(panel_vignette)
    panel_vignette_draw.ellipse((70, 570, 770, 1220), fill=(10, 16, 26, 82))
    panel_vignette = panel_vignette.filter(ImageFilter.GaussianBlur(78))
    panel = Image.alpha_composite(panel, panel_vignette)

    panel_border = Image.new("RGBA", (ICON_SIZE, ICON_SIZE), (0, 0, 0, 0))
    panel_border_draw = ImageDraw.Draw(panel_border)
    panel_border_draw.rounded_rectangle(
        panel_bounds,
        radius=panel_radius,
        outline=(116, 134, 165, 165),
        width=14,
    )
    panel_border_draw.rounded_rectangle(
        (panel_inset + 13, panel_inset + 13, ICON_SIZE - panel_inset - 13, ICON_SIZE - panel_inset - 13),
        radius=panel_radius - 13,
        outline=(28, 39, 60, 188),
        width=8,
    )
    panel_border_draw.rounded_rectangle(
        (panel_inset + 23, panel_inset + 23, ICON_SIZE - panel_inset - 23, ICON_SIZE - panel_inset - 23),
        radius=panel_radius - 23,
        outline=(190, 205, 230, 56),
        width=4,
    )
    panel = Image.alpha_composite(panel, panel_border)
    composite_masked(canvas, panel, panel_mask)

    font = ImageFont.truetype(str(FONT_PATH), 148)
    text = "DFC-GUI"
    draw = ImageDraw.Draw(canvas)

    bbox = draw.textbbox((0, 0), text, font=font, stroke_width=8)
    text_width = bbox[2] - bbox[0]
    text_height = bbox[3] - bbox[1]
    x = (ICON_SIZE - text_width) / 2
    y = (ICON_SIZE - text_height) / 2 - 2

    draw.text(
        (x, y + 11),
        text,
        font=font,
        fill=(47, 53, 66, 148),
        stroke_width=10,
        stroke_fill=(32, 37, 48, 92),
    )
    draw.text(
        (x, y),
        text,
        font=font,
        fill=(248, 249, 250, 255),
        stroke_width=8,
        stroke_fill=(141, 143, 147, 224),
    )

    text_glow = Image.new("RGBA", (ICON_SIZE, ICON_SIZE), (0, 0, 0, 0))
    glow_draw = ImageDraw.Draw(text_glow)
    glow_draw.text(
        (x, y - 4),
        text,
        font=font,
        fill=(255, 255, 255, 92),
        stroke_width=0,
    )
    text_glow = text_glow.filter(ImageFilter.GaussianBlur(16))
    canvas.alpha_composite(text_glow)

    text_highlight = Image.new("RGBA", (ICON_SIZE, ICON_SIZE), (0, 0, 0, 0))
    text_highlight_draw = ImageDraw.Draw(text_highlight)
    text_highlight_draw.text(
        (x, y - 6),
        text,
        font=font,
        fill=(255, 255, 255, 54),
        stroke_width=0,
    )
    text_highlight = text_highlight.filter(ImageFilter.GaussianBlur(10))
    canvas.alpha_composite(text_highlight)

    draw = ImageDraw.Draw(canvas)
    draw.text(
        (x, y),
        text,
        font=font,
        fill=(248, 249, 250, 255),
        stroke_width=8,
        stroke_fill=(141, 143, 147, 224),
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
