#!/usr/bin/env python3
"""Make a square black-on-white QR code PNG for a pack's end card.

Usage: tools/make_qr.py <https-url> <out.png> [--size 224]
"""
import argparse
import sys

try:
    import qrcode
    from PIL import Image
except ImportError:
    sys.exit("missing modules; run: pip install -r tools/requirements-kit.txt")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("url", help="must start with https://")
    ap.add_argument("out", help="output PNG path")
    ap.add_argument("--size", type=int, default=224, help="width and height in pixels")
    args = ap.parse_args()

    if not args.url.startswith("https://"):
        print("error: the URL must start with https://", file=sys.stderr)
        return 1
    if args.size < 1:
        print("error: --size must be positive", file=sys.stderr)
        return 1

    qr = qrcode.QRCode(error_correction=qrcode.constants.ERROR_CORRECT_M, border=2)
    qr.add_data(args.url)
    qr.make(fit=True)
    img = qr.make_image(fill_color="black", back_color="white").convert("RGB")
    img = img.resize((args.size, args.size), Image.NEAREST)
    img.save(args.out, "PNG")
    print(f"wrote {args.out} ({args.size}x{args.size})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
