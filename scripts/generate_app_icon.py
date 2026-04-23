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
    logging.info("Rendering %sx%s icon source", ICON_SIZE, ICON_SIZE)
    canvas = Image.new("RGBA", (ICON_SIZE, ICON_SIZE), (0, 0, 0, 0))

    outer_mask = rounded_rect_mask(ICON_SIZE, inset=78, radius=226)
    outer_shadow = Image.new("RGBA", (ICON_SIZE, ICON_SIZE), (0, 0, 0, 0))
    outer_shadow.paste((30, 30, 34, 165), mask=outer_mask)
    outer_shadow = outer_shadow.filter(ImageFilter.GaussianBlur(46))
    canvas.alpha_composite(outer_shadow, dest=(0, 34))

    outer = build_vertical_gradient(ICON_SIZE, (249, 249, 250), (215, 212, 206))
    outer = Image.alpha_composite(
        outer,
        build_radial_highlight(ICON_SIZE, (250, 210), 520, (255, 255, 255)),
    )
    outer = Image.alpha_composite(
        outer,
        build_radial_highlight(ICON_SIZE, (860, 850), 560, (183, 181, 176)),
    )
    shell_gloss = Image.new("RGBA", (ICON_SIZE, ICON_SIZE), (0, 0, 0, 0))
    shell_gloss_draw = ImageDraw.Draw(shell_gloss)
    shell_gloss_draw.pieslice(
        (-40, -220, 1120, 760),
        start=12,
        end=155,
        fill=(255, 255, 255, 54),
    )
    shell_gloss = shell_gloss.filter(ImageFilter.GaussianBlur(36))
    outer = Image.alpha_composite(outer, shell_gloss)

    outer_border = Image.new("RGBA", (ICON_SIZE, ICON_SIZE), (0, 0, 0, 0))
    outer_border_draw = ImageDraw.Draw(outer_border)
    outer_border_draw.rounded_rectangle(
        (78, 78, 946, 946),
        radius=226,
        outline=(255, 255, 255, 175),
        width=12,
    )
    outer_border_draw.rounded_rectangle(
        (92, 92, 932, 932),
        radius=212,
        outline=(165, 162, 157, 78),
        width=10,
    )
    outer = Image.alpha_composite(outer, outer_border)
    composite_masked(canvas, outer, outer_mask)

    inner_mask = rounded_rect_mask(ICON_SIZE, inset=205, radius=138)
    inner_shadow = Image.new("RGBA", (ICON_SIZE, ICON_SIZE), (0, 0, 0, 0))
    inner_shadow.paste((14, 18, 28, 150), mask=inner_mask)
    inner_shadow = inner_shadow.filter(ImageFilter.GaussianBlur(34))
    canvas.alpha_composite(inner_shadow, dest=(0, 26))

    inner = build_vertical_gradient(ICON_SIZE, (74, 92, 123), (24, 35, 57))
    inner = Image.alpha_composite(
        inner,
        build_radial_highlight(ICON_SIZE, (335, 315), 245, (119, 151, 210)),
    )
    inner = Image.alpha_composite(
        inner,
        build_radial_highlight(ICON_SIZE, (768, 726), 290, (87, 127, 198)),
    )
    inner = Image.alpha_composite(
        inner,
        build_radial_highlight(ICON_SIZE, (760, 405), 270, (188, 202, 224)),
    )

    panel_gloss = Image.new("RGBA", (ICON_SIZE, ICON_SIZE), (0, 0, 0, 0))
    panel_gloss_draw = ImageDraw.Draw(panel_gloss)
    panel_gloss_draw.pieslice(
        (40, -40, 1120, 830),
        start=20,
        end=136,
        fill=(255, 255, 255, 34),
    )
    panel_gloss_draw.ellipse((420, 126, 970, 540), fill=(255, 255, 255, 18))
    panel_gloss = panel_gloss.filter(ImageFilter.GaussianBlur(52))
    inner = Image.alpha_composite(inner, panel_gloss)

    panel_vignette = Image.new("RGBA", (ICON_SIZE, ICON_SIZE), (0, 0, 0, 0))
    panel_vignette_draw = ImageDraw.Draw(panel_vignette)
    panel_vignette_draw.ellipse((120, 560, 760, 1200), fill=(10, 16, 26, 72))
    panel_vignette = panel_vignette.filter(ImageFilter.GaussianBlur(78))
    inner = Image.alpha_composite(inner, panel_vignette)

    inner_border = Image.new("RGBA", (ICON_SIZE, ICON_SIZE), (0, 0, 0, 0))
    inner_border_draw = ImageDraw.Draw(inner_border)
    inner_border_draw.rounded_rectangle(
        (205, 205, 819, 819),
        radius=138,
        outline=(116, 134, 165, 165),
        width=12,
    )
    inner_border_draw.rounded_rectangle(
        (216, 216, 808, 808),
        radius=128,
        outline=(28, 39, 60, 188),
        width=8,
    )
    inner_border_draw.rounded_rectangle(
        (224, 224, 800, 800),
        radius=120,
        outline=(190, 205, 230, 48),
        width=4,
    )
    inner = Image.alpha_composite(inner, inner_border)
    composite_masked(canvas, inner, inner_mask)

    font = ImageFont.truetype(str(FONT_PATH), 126)
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
