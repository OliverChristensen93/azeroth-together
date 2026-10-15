"""Check that creature displays have their model in the game data.

    python -I tools/spawns/check_displays.py <mpqx.exe> <WoW/Data> <display id>...

Exits 1 if any display's model is missing (the client would draw nothing for it).
"""
import os
import struct
import subprocess
import sys
import tempfile


def rows(path):
    data = open(path, 'rb').read()
    _, count, _, size, _ = struct.unpack('<4s4I', data[:20])
    records = data[20:20 + count * size]
    strings = data[20 + count * size:]
    for r in range(count):
        yield records[r * size:(r + 1) * size], strings


def u32(row, field):
    return struct.unpack_from('<I', row, field * 4)[0]


def text(strings, offset):
    return strings[offset:strings.index(b'\0', offset)].decode('latin-1')


def main():
    mpqx, data, ids = sys.argv[1], sys.argv[2], [int(a) for a in sys.argv[3:]]
    tmp = tempfile.mkdtemp()

    def extract(vpath, out):
        return subprocess.run([mpqx, data, vpath, out], capture_output=True).returncode == 0

    cdi, cmd = os.path.join(tmp, 'cdi.dbc'), os.path.join(tmp, 'cmd.dbc')
    assert extract('DBFilesClient\\CreatureDisplayInfo.dbc', cdi)
    assert extract('DBFilesClient\\CreatureModelData.dbc', cmd)
    model_of = {u32(r, 0): u32(r, 1) for r, _ in rows(cdi)}
    path_of = {u32(r, 0): text(s, u32(r, 2)) for r, s in rows(cmd)}
    missing = 0
    for d in ids:
        model = model_of.get(d)
        path = path_of.get(model, '')
        m2 = path[:-4] + '.m2' if path.lower().endswith('.mdx') else path
        ok = bool(m2) and extract(m2, os.path.join(tmp, 'model.m2'))
        print(f"{d}: model {model} {m2 or '?'} {'ok' if ok else 'MISSING'}")
        missing += not ok
    sys.exit(1 if missing else 0)


main()
