import os
from PIL import Image

icon_dir = "widgets/button/icons"
for filename in os.listdir(icon_dir):
    if filename.endswith(".png"):
        filepath = os.path.join(icon_dir, filename)
        img = Image.open(filepath)
        print(f"{filename}: {img.mode}")
        if img.mode == 'RGBA':
            extrema = img.getextrema()
            print(f"  Extrema: {extrema}")
            alpha = img.split()[3]
            print(f"  Alpha min/max: {alpha.getextrema()}")

    if filename.endswith(".bmp"):
        filepath = os.path.join(icon_dir, filename)
        with open(filepath, "rb") as f:
            data = f.read()
            # Check BMP header
            if data[:2] != b'BM':
                print(f"{filename}: Not a BMP")
                continue
            
            offset = int.from_bytes(data[10:14], 'little')
            width = int.from_bytes(data[18:22], 'little')
            height = int.from_bytes(data[22:26], 'little')
            bpp = int.from_bytes(data[28:30], 'little')
            
            print(f"{filename}: {width}x{height} {bpp}bpp Offset:{offset}")
            
            if bpp != 32:
                print(f"  Warning: Not 32bpp")
                continue
                
            # Check some pixels for alpha
            # Pixel data starts at offset
            # 32bpp is usually BGRA
            
            pixel_data = data[offset:]
            total_pixels = width * abs(height)
            
            alphas = []
            for i in range(0, len(pixel_data), 4):
                if i+3 < len(pixel_data):
                    alphas.append(pixel_data[i+3])
            
            min_a = min(alphas) if alphas else 0
            max_a = max(alphas) if alphas else 0
            avg_a = sum(alphas)/len(alphas) if alphas else 0
            
            print(f"  Alpha: min={min_a}, max={max_a}, avg={avg_a:.2f}")
            
            # Check if there are any transparent pixels
            transparent_count = sum(1 for a in alphas if a == 0)
            print(f"  Transparent pixels: {transparent_count}/{len(alphas)}")
