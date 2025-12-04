import os
from PIL import Image, ImageOps

icon_dir = "widgets/button/icons"
for filename in os.listdir(icon_dir):
    if filename.endswith(".png"):
        filepath = os.path.join(icon_dir, filename)
        try:
            img = Image.open(filepath)
            
            # Resize to 32x32 using high quality resampling
            img = img.resize((32, 32), Image.Resampling.LANCZOS)
            
            # Check if image has transparency
            if img.mode != 'RGBA' or img.getextrema()[3][0] == 255:
                # It's opaque. Assume Dark-on-White.
                # Convert to Grayscale
                gray = img.convert("L")
                # Invert so background (white) becomes black (0) -> transparent
                # and foreground (dark) becomes white (255) -> opaque
                mask = ImageOps.invert(gray)
                
                # Create a solid black image
                black = Image.new("RGBA", img.size, (0, 0, 0, 255))
                # Put the mask as alpha
                black.putalpha(mask)
                img = black
            else:
                # It has transparency. Ensure it's RGBA.
                img = img.convert("RGBA")
            
            name_part = os.path.splitext(filename)[0]
            if name_part.endswith("-outline"):
                name_part = name_part[:-8]
            
            bmp_path = os.path.join(icon_dir, name_part + ".bmp")
            
            # Save as BMP (32-bit)
            img.save(bmp_path, "BMP")
            print(f"Converted {filename} to {bmp_path}")
        except Exception as e:
            print(f"Failed to convert {filename}: {e}")
