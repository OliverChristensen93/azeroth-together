"""Bake Elwynn's creatures for extraction mode.

    python -I tools/spawns/bake_spawns.py <elwynn_spawns.json> content/spawns.ron

Keeps every permanent hostile or neutral creature of a kind in KINDS that stands inside the map's
outline, out of Goldshire's margin and not deep inside a mine (it would stand on the hill above),
and writes them as content/spawns.ron. See README.md for where the JSON comes from.
"""
import json
import math
import sys

# Creature name in the database -> key in content/enemies.ron.
KINDS = {
    'Murloc Forager': 'murloc_forager', 'Rockhide Boar': 'rockhide_boar',
    'Riverpaw Runt': 'riverpaw_runt', 'Mangy Wolf': 'mangy_wolf', 'Prowler': 'prowler',
    'Defias Thug': 'defias_thug', 'Defias Bandit': 'defias_bandit',
    'Gray Forest Wolf': 'gray_forest_wolf', 'Stonetusk Boar': 'stonetusk_boar',
    'Kobold Miner': 'kobold_miner', 'Young Forest Bear': 'young_forest_bear',
    'Kobold Worker': 'kobold_worker', 'Kobold Tunneler': 'kobold_tunneler', 'Murloc': 'murloc',
    'Kobold Vermin': 'kobold_vermin', 'Defias Cutpurse': 'defias_cutpurse',
    'Young Wolf': 'young_wolf', 'Defias Rogue Wizard': 'defias_rogue_wizard',
    'Timber Wolf': 'timber_wolf', 'Kobold Laborer': 'kobold_laborer',
    'Murloc Streamrunner': 'murloc_streamrunner', 'Forest Spider': 'forest_spider',
    'Longsnout': 'longsnout', 'Kobold Geomancer': 'kobold_geomancer',
    'Mine Spider': 'mine_spider',
}

# Keep in step with content/extraction.ron's `boundary` and `safe`.
OUTLINE = [
    (-9135.0, 820.0), (-9135.0, 300.0), (-8850.0, 300.0), (-8850.0, -1300.0),
    (-9300.0, -1480.0), (-9950.0, -1480.0), (-9950.0, -400.0), (-10150.0, -100.0),
    (-10150.0, 700.0), (-9700.0, 820.0),
]
SAFE = (-9480.1, 63.5)
SAFE_MARGIN = 90.0
# A mine creature this far below the open-air creatures around it is underground.
UNDERGROUND = 8.0


def inside(x, y):
    hit = False
    j = len(OUTLINE) - 1
    for i in range(len(OUTLINE)):
        (ax, ay), (bx, by) = OUTLINE[i], OUTLINE[j]
        if (ay > y) != (by > y) and x < (bx - ax) * (y - ay) / (by - ay) + ax:
            hit = not hit
        j = i
    return hit


def underground(s, surface):
    if 'Mine' not in s.get('subzone', ''):
        return False
    near = sorted(t['z'] for t in surface if math.hypot(t['x'] - s['x'], t['y'] - s['y']) < 80)
    return not near or s['z'] < near[len(near) // 2] - UNDERGROUND


def path_of(s):
    # 'waypoints', 'waypoints (linear, back and forth)', or a formation leader's of either.
    movement = s.get('movement') or ''
    if 'waypoints' not in movement:
        return []
    points = [(float(p['x']), float(p['y']))
              for p in sorted(s.get('waypoints') or [], key=lambda p: p['point'])]
    length = sum(math.dist(a, b) for a, b in zip(points, points[1:]))
    if len(points) < 2 or length < 5:
        return []
    if 'linear' in movement:
        points += points[-2:0:-1]
    return points


def main():
    spawns = json.load(open(sys.argv[1], encoding='utf-8'))
    pool = [
        s for s in spawns
        if s.get('zone') == 'Elwynn Forest' and s.get('permanent')
        and s['reaction_to_alliance'] in ('hostile', 'neutral') and s['rank'] == 0
        and s['name'] in KINDS
    ]
    surface = [s for s in pool if 'Mine' not in s.get('subzone', '')]
    kept, dropped = [], {'outline': 0, 'goldshire': 0, 'underground': 0}
    for s in pool:
        if not inside(s['x'], s['y']):
            dropped['outline'] += 1
        elif math.dist((s['x'], s['y']), SAFE) < SAFE_MARGIN:
            dropped['goldshire'] += 1
        elif underground(s, surface):
            dropped['underground'] += 1
        else:
            kept.append(s)
    kept.sort(key=lambda s: (KINDS[s['name']], s['x'], s['y']))
    out = [
        '// Elwynn Forest\'s creatures for extraction mode, baked by tools/spawns/bake_spawns.py from',
        '// the CMaNGOS Classic DB (ClassicDB_1_12_1_z2815, GPL-3.0; see tools/spawns/README.md).',
        '// Do not edit by hand: change the bake and run it again.',
        '[',
        '    (map: "elwynn", spawns: [',
    ]
    for s in kept:
        line = (f'        (enemy: "{KINDS[s["name"]]}", pos: ({float(s["x"]):.1f}, {float(s["y"]):.1f}), '
                f'wander: {float(s.get("wander_distance") or 0):.1f}, '
                f'respawn: {float(s["spawn_time_s"][0]):.1f}')
        path = path_of(s)
        if path:
            line += ', path: [' + ', '.join(f'({x:.1f}, {y:.1f})' for x, y in path) + ']'
        out.append(line + '),')
    out += ['    ]),', ']', '']
    open(sys.argv[2], 'w', encoding='utf-8', newline='\n').write('\n'.join(out))
    counts = {}
    for s in kept:
        counts[KINDS[s['name']]] = counts.get(KINDS[s['name']], 0) + 1
    print(f'kept {len(kept)} of {len(pool)}; left out {dropped}')
    for k, n in sorted(counts.items(), key=lambda kv: -kv[1]):
        print(f'  {n:4} {k}')
    print(f'  patrols: {sum(1 for s in kept if path_of(s))}')


main()
