#!/usr/bin/env python3
"""Generate TP5 Pix texture for golf ball - TEXT ONLY (logos are geometry)."""

from PIL import Image, ImageDraw, ImageFont
import math
import os

TEXTURE_SIZE = 2048


def get_font(size):
    """Get a font, falling back to default if needed."""
    try:
        return ImageFont.truetype("/System/Library/Fonts/Helvetica.ttc", size)
    except:
        return ImageFont.load_default()


def draw_tp5_text(draw, cx, cy, scale=1.0):
    """Draw TP5 text with lines on either side."""
    black = (0, 0, 0)
    font_size = int(45 * scale)
    font = get_font(font_size)

    text = "TP5"
    bbox = draw.textbbox((0, 0), text, font=font)
    text_w = bbox[2] - bbox[0]
    text_h = bbox[3] - bbox[1]
    draw.text((cx - text_w//2, cy - text_h//2 - 5), text, fill=black, font=font)

    # Lines on either side
    line_len = int(80 * scale)
    line_width = int(5 * scale)
    gap = int(45 * scale)
    draw.line([(cx - gap - line_len, cy), (cx - gap, cy)], fill=black, width=line_width)
    draw.line([(cx + gap, cy), (cx + gap + line_len, cy)], fill=black, width=line_width)


def draw_taylormade_text(draw, cx, cy, scale=1.0):
    """Draw TaylorMade text."""
    black = (0, 0, 0)
    font_size = int(32 * scale)
    font = get_font(font_size)
    text = "TaylorMade"
    bbox = draw.textbbox((0, 0), text, font=font)
    text_w = bbox[2] - bbox[0]
    text_h = bbox[3] - bbox[1]
    draw.text((cx - text_w//2, cy - text_h//2), text, fill=black, font=font)


def draw_number(draw, cx, cy, num="1", scale=1.0):
    """Draw a ball number."""
    black = (0, 0, 0)
    font_size = int(70 * scale)
    font = get_font(font_size)
    bbox = draw.textbbox((0, 0), num, font=font)
    text_w = bbox[2] - bbox[0]
    text_h = bbox[3] - bbox[1]
    draw.text((cx - text_w//2, cy - text_h//2), num, fill=black, font=font)


def lat_lon_to_uv(lat_deg, lon_deg):
    """Convert latitude/longitude to UV coordinates."""
    u = (lon_deg % 360) / 360.0
    v = (90 - lat_deg) / 180.0
    return u, v


def create_tp5_pix_texture(filepath):
    """Create TP5 Pix texture with TEXT ONLY (logos are geometry)."""
    size = TEXTURE_SIZE
    img = Image.new('RGB', (size, size), (255, 255, 255))  # White background
    draw = ImageDraw.Draw(img)

    equator_y = size * 0.5  # v=0.5 is equator

    # TP5 text with lines - at longitude 90°
    tp5_lon = 90
    u, _ = lat_lon_to_uv(0, tp5_lon)
    draw_tp5_text(draw, u * size, equator_y, scale=1.2)

    # TaylorMade + number - at longitude 240° (aligned with upper row logo)
    tm_lon = 240
    u, _ = lat_lon_to_uv(0, tm_lon)
    draw_taylormade_text(draw, u * size, equator_y, scale=1.3)
    draw_number(draw, u * size, equator_y + 80, "1", scale=1.0)

    img.save(filepath)
    print(f"Created: {filepath}")
    return filepath


if __name__ == "__main__":
    script_dir = os.path.dirname(os.path.abspath(__file__))
    texture_path = os.path.join(script_dir, "tp5_pix_texture.png")
    create_tp5_pix_texture(texture_path)
