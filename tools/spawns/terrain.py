"""The ground height of Eastern Kingdoms (map 0) terrain, read from its ADT tiles with mpqx.

    ground = Ground(<mpqx.exe>, <WoW/Data>)
    ground.height(x, y)  # world yards; None where there is no tile

Each tile's 16 x 16 MCNK chunks carry a base height (the header's position z) and a 9 x 9 grid of
outer heights in MCVT (interleaved with 8 x 8 inner ones); the height between them is bilinear.
"""
import math
import os
import struct
import subprocess
import tempfile

TILE = 1600.0 / 3.0
CHUNK = TILE / 16.0


class Ground:
    def __init__(self, mpqx, data):
        self.mpqx, self.data = mpqx, data
        self.tmp = tempfile.mkdtemp()
        self.tiles = {}

    def _tile(self, a, b):
        if (a, b) not in self.tiles:
            out = os.path.join(self.tmp, f'{a}_{b}.adt')
            vpath = f'World\\Maps\\Azeroth\\Azeroth_{a}_{b}.adt'
            ok = subprocess.run([self.mpqx, self.data, vpath, out], capture_output=True).returncode == 0
            self.tiles[(a, b)] = chunks(open(out, 'rb').read()) if ok else {}
        return self.tiles[(a, b)]

    def height(self, x, y):
        # Tile and chunk indices run against the world axes: a and the chunk's IndexX from y,
        # b and IndexY from x.
        fa, fb = 32 - y / TILE, 32 - x / TILE
        a, b = math.floor(fa), math.floor(fb)
        cx, cy = (fa - a) * 16, (fb - b) * 16
        ix, iy = min(15, int(cx)), min(15, int(cy))
        chunk = self._tile(a, b).get((ix, iy))
        if chunk is None:
            return None
        base, mcvt = chunk
        # Within the chunk, MCVT's rows run along IndexY (world x) and its columns along IndexX.
        col, row = (cx - ix) * 8, (cy - iy) * 8
        c, r = min(7, int(col)), min(7, int(row))
        fc, fr = col - c, row - r

        def outer(r, c):
            return mcvt[r * 17 + c]

        top = outer(r, c) * (1 - fc) + outer(r, c + 1) * fc
        bottom = outer(r + 1, c) * (1 - fc) + outer(r + 1, c + 1) * fc
        return base + top * (1 - fr) + bottom * fr


def chunks(adt):
    """{(IndexX, IndexY): (base height, 145 MCVT heights)} for every MCNK in an ADT file."""
    out = {}
    p = 0
    while p + 8 <= len(adt):
        tag, size = struct.unpack_from('<4sI', adt, p)
        if tag == b'KNCM':
            ix, iy = struct.unpack_from('<II', adt, p + 8 + 0x04)
            of_height = struct.unpack_from('<I', adt, p + 8 + 0x14)[0]
            base = struct.unpack_from('<3f', adt, p + 8 + 0x68)[2]
            mcvt = struct.unpack_from('<145f', adt, p + of_height + 8)
            out[(ix, iy)] = (base, mcvt)
        p += 8 + size
    return out
