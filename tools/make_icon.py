"""Generate a simple clock icon (multi-size .ico) with analytic antialiasing.

No dependencies beyond the stdlib: pixels are coverage values, the ICO is
written as uncompressed 32bpp BGRA entries (what Windows has supported forever).
"""
import math
import struct
import sys

SIZES = [16, 24, 32, 48, 64, 128, 256]

DISC = (0x24, 0x2A, 0x38)      # dark slate dial
RING = (0xD8, 0xDE, 0xE9)      # light ring so it reads on dark backgrounds
HAND = (0xFF, 0xFF, 0xFF)      # hour + minute hands
SECOND = (0xFF, 0xD8, 0x8A)    # gold second hand - the app's default accent


def clamp01(v):
    return 0.0 if v < 0.0 else (1.0 if v > 1.0 else v)


def over(dst, src):
    """dst, src = (r, g, b, a) with a in 0..1. Returns src over dst."""
    sa = src[3]
    da = dst[3]
    out_a = sa + da * (1.0 - sa)
    if out_a <= 0.0:
        return (0.0, 0.0, 0.0, 0.0)
    return (
        (src[0] * sa + dst[0] * da * (1.0 - sa)) / out_a,
        (src[1] * sa + dst[1] * da * (1.0 - sa)) / out_a,
        (src[2] * sa + dst[2] * da * (1.0 - sa)) / out_a,
        out_a,
    )


def seg_coverage(px, py, x0, y0, x1, y1, half_w):
    """Coverage of a round-capped segment of half-width half_w at (px, py)."""
    dx = x1 - x0
    dy = y1 - y0
    length2 = dx * dx + dy * dy
    if length2 <= 0.0:
        t = 0.0
    else:
        t = ((px - x0) * dx + (py - y0) * dy) / length2
        t = 0.0 if t < 0.0 else (1.0 if t > 1.0 else t)
    cx = x0 + t * dx
    cy = y0 + t * dy
    d = math.hypot(px - cx, py - cy)
    return clamp01(half_w - d + 0.5)


def hand_end(cx, cy, length, degrees):
    """Angle measured clockwise from 12 o'clock."""
    rad = math.radians(degrees)
    return cx + length * math.sin(rad), cy - length * math.cos(rad)


def render(size):
    cx = cy = size / 2.0
    r_out = size / 2.0 - max(0.5, size * 0.015)
    ring_w = max(1.0, r_out * 0.10)
    r_in = r_out - ring_w

    hour_len = r_in * 0.50
    minute_len = r_in * 0.74
    second_len = r_in * 0.82
    hour_w = max(1.0, r_in * 0.11)
    minute_w = max(1.0, r_in * 0.075)
    second_w = max(0.5, r_in * 0.035)
    dot_r = max(1.0, r_in * 0.075)

    draw_second = size >= 32

    hx, hy = hand_end(cx, cy, hour_len, 305.0)      # 10:10 look
    mx, my = hand_end(cx, cy, minute_len, 60.0)
    sx, sy = hand_end(cx, cy, second_len, 200.0)

    rows = []
    for y in range(size):
        row = []
        for x in range(size):
            px = x + 0.5
            py = y + 0.5
            d = math.hypot(px - cx, py - cy)

            # dial
            disc_cov = clamp01(r_out - d + 0.5)
            # ring: fill the annulus, i.e. disc coverage minus inner disc coverage
            inner_cov = clamp01(r_in - d + 0.5)
            ring_cov = disc_cov - inner_cov

            if disc_cov > 0.0:
                px_col = (float(DISC[0]), float(DISC[1]), float(DISC[2]), disc_cov)
            else:
                px_col = (0.0, 0.0, 0.0, 0.0)

            if ring_cov > 0.0:
                px_col = over(px_col, (RING[0], RING[1], RING[2], ring_cov))

            if draw_second:
                sc = seg_coverage(px, py, cx, cy, sx, sy, second_w)
                if sc > 0.0:
                    px_col = over(px_col, (SECOND[0], SECOND[1], SECOND[2], sc))

            hc = seg_coverage(px, py, cx, cy, hx, hy, hour_w)
            if hc > 0.0:
                px_col = over(px_col, (HAND[0], HAND[1], HAND[2], hc))
            mc = seg_coverage(px, py, cx, cy, mx, my, minute_w)
            if mc > 0.0:
                px_col = over(px_col, (HAND[0], HAND[1], HAND[2], mc))

            dc = clamp01(dot_r - d + 0.5)
            if dc > 0.0:
                px_col = over(px_col, (SECOND[0], SECOND[1], SECOND[2], dc))

            row.append(px_col)
        rows.append(row)
    return rows


def to_bmp(rows, size):
    """BITMAPINFOHEADER + bottom-up BGRA + empty AND mask."""
    hdr = struct.pack('<IiiHHIIiiII', 40, size, size * 2, 1, 32, 0, size * size * 4, 0, 0, 0, 0)
    body = bytearray()
    for y in range(size - 1, -1, -1):
        for x in range(size):
            r, g, b, a = rows[y][x]
            body += bytes((int(b + 0.5), int(g + 0.5), int(r + 0.5), int(a * 255.0 + 0.5)))
    mask_row = ((size + 31) // 32) * 4
    body += bytes(mask_row * size)  # AND mask: all zero, alpha does the work
    return hdr + bytes(body)


def main(path):
    images = [to_bmp(render(s), s) for s in SIZES]
    out = bytearray(struct.pack('<HHH', 0, 1, len(SIZES)))
    offset = 6 + 16 * len(SIZES)
    for size, data in zip(SIZES, images):
        dim = 0 if size >= 256 else size
        out += struct.pack('<BBBBHHII', dim, dim, 0, 0, 1, 32, len(data), offset)
        offset += len(data)
    for data in images:
        out += data
    with open(path, 'wb') as fh:
        fh.write(out)
    print('wrote %s (%d bytes, %d sizes)' % (path, len(out), len(SIZES)))


if __name__ == '__main__':
    main(sys.argv[1] if len(sys.argv) > 1 else 'clock.ico')
