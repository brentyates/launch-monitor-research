#!/usr/bin/env python3
"""Generate a realistic artificial turf hitting mat texture for top-down camera view."""

from PIL import Image, ImageFilter
import random
import os

def generate_turf_texture(width=512, height=512, seed=42):
    random.seed(seed)

    base_r, base_g, base_b = 28, 58, 32

    img = Image.new('RGB', (width, height))
    pixels = img.load()

    for y in range(height):
        for x in range(width):
            noise = random.gauss(0, 4)
            r = int(max(0, min(255, base_r + noise + random.gauss(0, 2))))
            g = int(max(0, min(255, base_g + noise + random.gauss(0, 3))))
            b = int(max(0, min(255, base_b + noise + random.gauss(0, 2))))
            pixels[x, y] = (r, g, b)

    img = img.filter(ImageFilter.GaussianBlur(radius=0.5))
    pixels = img.load()

    for _ in range(width * height // 20):
        x = random.randint(0, width - 1)
        y = random.randint(0, height - 1)

        current = pixels[x, y]
        brightness = random.choice([-8, -6, 6, 8])
        new_color = (
            max(0, min(255, current[0] + brightness)),
            max(0, min(255, current[1] + brightness + random.randint(-2, 2))),
            max(0, min(255, current[2] + brightness))
        )
        pixels[x, y] = new_color

    return img


if __name__ == '__main__':
    script_dir = os.path.dirname(os.path.abspath(__file__))
    output_path = os.path.join(
        script_dir, '..', 'unity', 'LaunchSimulator', 'Assets', 'Resources', 'artificial_turf.png'
    )

    img = generate_turf_texture(512, 512)
    img.save(output_path)
    print(f'Generated {output_path}')
