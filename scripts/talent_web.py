"""Writes content/talents.ron: the first layout of the talent web.

The RON file is the source of truth once written: edit it by hand freely. This script is how its
first version was laid out (a ring of nine classes, a shared cluster between each pair of
neighbours, an inner ring that joins them all, keystones on the rim), kept so the layout can be
regenerated after big changes. Running it OVERWRITES content/talents.ron.

    python scripts/talent_web.py

See docs/TALENT-WEB.md for the design.
"""

import math
import os

# The ring, clockwise from the top. Neighbours share a cluster.
CLASSES = ["warrior", "rogue", "hunter", "druid", "shaman", "mage", "warlock", "priest", "paladin"]
NAMES = {c: c.capitalize() for c in CLASSES}

R_START = 430
R_RING = 170


def stat(s, v):
    return f"Stat({s}, {v})"


def tagged(t, b, v):
    return f"Tagged({t}, {b}, {v})"


# Small nodes: (name, effect).
def dmg(t, v=0.01):
    return (f"{t} Damage" if t not in ("DamageOverTime",) else "Lingering Pain", tagged(t, "Damage", v))


def crit(t, v=0.005):
    return (f"{t} Precision" if t != "DamageOverTime" else "Festering", tagged(t, "Crit", v))


SMALL = {
    "might": ("Might", stat("Damage", 0.01)),
    "haste": ("Haste", stat("Cooldown", 0.01)),
    "reach": ("Reach", stat("Area", 0.02)),
    "swift": ("Swiftness", stat("Speed", 0.01)),
    "vigor": ("Vigor", stat("Health", 0.03)),
    "renew": ("Renewal", stat("Regen", 0.2)),
    "armor": ("Toughness", stat("Armor", 0.015)),
    "magpie": ("Magpie", stat("Pickup", 0.08)),
    "luck": ("Fortune", stat("Luck", 0.04)),
    "wisdom": ("Wisdom", stat("XpGain", 0.03)),
    "greed": ("Greed", stat("GoldGain", 0.05)),
    "keen": ("Keen Eye", stat("Crit", 0.005)),
    "fero": ("Ferocity", stat("CritDamage", 0.05)),
}


def name_of(effect_kind, tag):
    return f"{tag} {effect_kind}"


# Each class: its start's three spoke nodes toward the hub, then its two branches (toward the
# previous class, "p", and the next, "n"): three smalls and a notable each, and a deep notable
# joining them.
CLASS_TREES = {
    "warrior": dict(
        spoke=[dmg("Physical"), SMALL["vigor"], SMALL["armor"]],
        p=([("Thunder Reach", tagged("Area", "Area", 0.02))] * 3,
           ("Blood and Thunder", [tagged("Area", "Damage", 0.03), tagged("Physical", "Area", 0.06)])),
        n=([dmg("Melee")] * 3,
           ("Mortal Wounds", ["Execute(0.15)", tagged("Melee", "CritDamage", 0.1)])),
        deep=("Battle Trance", ["KillHaste(0.03)", stat("Regen", 0.6)]),
    ),
    "rogue": dict(
        spoke=[crit("Melee"), SMALL["swift"], SMALL["greed"]],
        p=([crit("Melee")] * 3,
           ("Blade Dance", [tagged("Melee", "Cooldown", 0.03), stat("Speed", 0.03)])),
        n=([dmg("DamageOverTime")] * 3,
           ("Murder", ["Execute(0.2)", tagged("DamageOverTime", "Damage", 0.01)])),
        deep=("Ruthlessness", [tagged("Stun", "Damage", 0.05), tagged("Melee", "Crit", 0.01)]),
    ),
    "hunter": dict(
        spoke=[dmg("Projectile"), SMALL["magpie"], SMALL["swift"]],
        p=([dmg("Projectile")] * 3,
           ("Trueshot", [tagged("Projectile", "Crit", 0.02), tagged("Projectile", "Range", 0.1)])),
        n=([dmg("Nature")] * 3,
           ("Serpent's Bite", ["CritBurn(Nature, 0.3)"])),
        deep=("Rapid Fire", [tagged("Projectile", "Cooldown", 0.03), "KillHaste(0.03)"]),
    ),
    "druid": dict(
        spoke=[dmg("Nature"), SMALL["renew"], SMALL["vigor"]],
        p=([dmg("Arcane")] * 3,
           ("Moonglow", [tagged("Arcane", "Cooldown", 0.03), tagged("Nature", "Cooldown", 0.02)])),
        n=([("Lasting Swarm", tagged("DamageOverTime", "Duration", 0.05))] * 3,
           ("Insect Plague", [tagged("DamageOverTime", "Duration", 0.15), tagged("DamageOverTime", "Damage", 0.01)])),
        deep=("Eclipse", ["Pair(Nature, Arcane, 0.02)"]),
    ),
    "shaman": dict(
        spoke=[dmg("Nature"), SMALL["armor"], SMALL["renew"]],
        p=([dmg("Nature")] * 3,
           ("Elemental Fury", [tagged("Nature", "CritDamage", 0.25), tagged("Nature", "Crit", 0.01)])),
        n=([dmg("Totem", 0.015)] * 3,
           ("Totemic Focus", [tagged("Totem", "Cooldown", 0.03), tagged("Totem", "Area", 0.06)])),
        deep=("Storm, Earth and Fire", ["Pair(Nature, Fire, 0.02)"]),
    ),
    "mage": dict(
        spoke=[dmg("Arcane"), SMALL["haste"], SMALL["wisdom"]],
        p=([dmg("Frost")] * 3,
           ("Deep Winter", ["ChillFreeze(0.15)"])),
        n=([dmg("Fire")] * 3,
           ("Fanned Flames", ["CritBurn(Fire, 0.4)"])),
        deep=("Frostfire Mastery", ["Hybrid(Fire, Frost, Damage, 0.05)", "AugmentWeight(Frostfire, 1.0)"]),
    ),
    "warlock": dict(
        spoke=[dmg("Shadow"), SMALL["vigor"], SMALL["renew"]],
        p=([dmg("Fire")] * 3,
           ("Ruin", [tagged("Shadow", "CritDamage", 0.3), tagged("Fire", "CritDamage", 0.3)])),
        n=([dmg("DamageOverTime")] * 3,
           ("Malediction", [tagged("DamageOverTime", "Duration", 0.2), tagged("Shadow", "Damage", 0.01)])),
        deep=("Shadowflame Pact", ["Pair(Shadow, Fire, 0.02)"]),
    ),
    "priest": dict(
        spoke=[dmg("Holy"), SMALL["renew"], SMALL["wisdom"]],
        p=([dmg("Shadow")] * 3,
           ("Shadow Weaving", ["CritBurn(Shadow, 0.3)"])),
        n=([dmg("Holy")] * 3,
           ("Divine Fury", ["CritStreak(Holy)"])),
        deep=("Twilight", ["Pair(Shadow, Holy, 0.02)"]),
    ),
    "paladin": dict(
        spoke=[dmg("Holy"), SMALL["armor"], SMALL["vigor"]],
        p=([dmg("Holy")] * 3,
           ("Vengeance", ["Pair(Holy, Physical, 0.02)"])),
        n=([SMALL["armor"], SMALL["vigor"], SMALL["armor"]],
           ("Holy Shield", [stat("Armor", 0.05), stat("Regen", 0.8)])),
        deep=("Crusader", ["KillHaste(0.03)", tagged("Holy", "Crit", 0.015)]),
    ),
}

# The cluster between class i and i+1: its two entries, an inner small and the way in from the
# hub, its core notable, an outer small, its steering node, and a keystone (or an outer
# notable), and maybe an extra notable beside the outer small.
CLUSTERS = [
    dict(  # warrior and rogue
        key="bloodletting",
        entries=[dmg("Physical"), dmg("Melee")],
        inner=SMALL["keen"],
        way_in=SMALL["vigor"],
        core=("Deep Wounds", ["CritBurn(Physical, 0.3)"]),
        outer=dmg("Physical"),
        steer=("Weapon Lore", ["OfferWeight(Melee, 0.3)", "Unlock(Concussive, [Melee])"]),
        tip=("Point Blank", "Keystone", ["Keystone(PointBlank)"]),
    ),
    dict(  # rogue and hunter
        key="precision",
        entries=[crit("Physical"), dmg("Projectile")],
        inner=SMALL["fero"],
        way_in=SMALL["swift"],
        core=("Lethal Precision", [tagged("Physical", "Crit", 0.015), stat("CritDamage", 0.1)]),
        outer=SMALL["keen"],
        steer=("Fletcher's Lore", ["OfferWeight(Projectile, 0.3)", "Unlock(Echo, [Projectile])"]),
        tip=("Steady Hand", "Keystone", ["Keystone(SteadyHand)"]),
    ),
    dict(  # hunter and druid
        key="wilds",
        entries=[dmg("Nature"), dmg("Nature")],
        inner=SMALL["magpie"],
        way_in=SMALL["swift"],
        core=("Venom Mastery", [tagged("Nature", "Damage", 0.03), tagged("DamageOverTime", "Damage", 0.02)]),
        outer=SMALL["renew"],
        steer=("Wild Lore", ["OfferWeight(Nature, 0.3)", "Unlock(Executioner, [DamageOverTime])"]),
        tip=("Survival Instincts", "Notable", [stat("Health", 0.08), stat("Regen", 0.5), stat("Speed", 0.02)]),
    ),
    dict(  # druid and shaman
        key="storms",
        entries=[dmg("Nature"), ("Storm Reach", tagged("Nature", "Area", 0.02))],
        inner=SMALL["reach"],
        way_in=SMALL["haste"],
        core=("Storm Caller", [tagged("Nature", "Area", 0.08), tagged("Nature", "Crit", 0.015)]),
        outer=SMALL["reach"],
        steer=("Storm Lore", ["Unlock(Concussive, [Area])", "AugmentWeight(Echo, 0.5)"]),
        tip=("Celestial Focus", "Notable", [tagged("Arcane", "Crit", 0.015), tagged("Nature", "Crit", 0.015)]),
    ),
    dict(  # shaman and mage
        key="winter",
        entries=[dmg("Frost"), dmg("Frost")],
        inner=crit("Frost"),
        way_in=SMALL["armor"],
        core=("Shattering Cold", ["FrozenCrit(0.25)"]),
        outer=("Biting Cold", tagged("Slow", "Damage", 0.01)),
        steer=("Glacial Lore", ["Unlock(Shatter, [Slow])", "OfferWeight(Frost, 0.3)"]),
        tip=("Elemental Equilibrium", "Keystone", ["Keystone(ElementalEquilibrium)"]),
        extra=("Thermal Shock", ["Thermal(0.2)"]),
    ),
    dict(  # mage and warlock
        key="inferno",
        entries=[dmg("Fire"), dmg("Fire")],
        inner=("Fire Reach", tagged("Fire", "Area", 0.02)),
        way_in=SMALL["might"],
        core=("Pyromaniac", [tagged("Fire", "Damage", 0.03), tagged("Fire", "Cooldown", 0.02)]),
        outer=dmg("Fire"),
        steer=("Fire Lore", ["OfferWeight(Fire, 0.3)", "Unlock(Ignite, [Fire])"]),
        tip=("Glass Cannon", "Keystone", ["Keystone(GlassCannon)"]),
    ),
    dict(  # warlock and priest
        key="shadows",
        entries=[dmg("Shadow"), dmg("Shadow")],
        inner=("Creeping Dark", tagged("DamageOverTime", "Duration", 0.05)),
        way_in=SMALL["renew"],
        core=("Shadow Mastery", [tagged("Shadow", "Damage", 0.03), tagged("DamageOverTime", "Damage", 0.02)]),
        outer=dmg("DamageOverTime"),
        steer=("Forbidden Lore", ["OfferWeight(Shadow, 0.3)", "AugmentWeight(Contagion, 1.0)"]),
        tip=("Slow Burn", "Keystone", ["Keystone(SlowBurn)"]),
    ),
    dict(  # priest and paladin
        key="light",
        entries=[dmg("Holy"), dmg("Holy")],
        inner=SMALL["vigor"],
        way_in=SMALL["renew"],
        core=("Spirit Link", ["Leech(0.1)"]),
        outer=crit("Holy"),
        steer=("Holy Lore", ["OfferWeight(Holy, 0.3)", "Rerolls(1)"]),
        tip=("Seal of Light", "Keystone", ["Keystone(SealOfLight)"]),
    ),
    dict(  # paladin and warrior
        key="bulwark",
        entries=[SMALL["armor"], dmg("Physical")],
        inner=SMALL["vigor"],
        way_in=SMALL["armor"],
        core=("Bulwark", [stat("Armor", 0.05), stat("Health", 0.06)]),
        outer=SMALL["vigor"],
        steer=("Battle Plans", ["Rerolls(1)", "OfferWeight(Physical, 0.3)"]),
        tip=("Juggernaut", "Keystone", ["Keystone(Juggernaut)"]),
    ),
]

# The inner ring: 18 nodes, one at each class's angle and one at each cluster's, starting at the
# warrior's and going clockwise. A few are notables.
RING = [
    SMALL["might"], SMALL["vigor"], SMALL["haste"], ("Quick Study", "N", [stat("XpGain", 0.06), "Rerolls(1)"]),
    SMALL["reach"], SMALL["swift"], SMALL["keen"], SMALL["luck"], SMALL["might"],
    ("Fortune's Favour", "N", [stat("Luck", 0.08), "Gold(25)"]), SMALL["haste"], SMALL["magpie"],
    SMALL["reach"], SMALL["vigor"], SMALL["fero"], ("Fleet of Foot", "N", [stat("Speed", 0.04), stat("Pickup", 0.15)]),
    SMALL["greed"], SMALL["wisdom"],
]

UTILITY = [SMALL["magpie"], SMALL["wisdom"], SMALL["luck"], SMALL["greed"], SMALL["renew"], SMALL["vigor"]]


nodes = []  # dicts: id, name, kind, class, pos, links, effects


def deg(a):
    return math.radians(a)


def place(r, angle):
    return (round(r * math.cos(deg(angle))), round(-r * math.sin(deg(angle))))


def add(id_, name, kind, r, angle, effects, cls=None):
    nodes.append(dict(id=id_, name=name, kind=kind, cls=cls, pos=place(r, angle), links=[], effects=effects))
    return id_


def link(a, b):
    for n in nodes:
        if n["id"] == a:
            n["links"].append(b)
            return
    raise KeyError(a)


def small(id_, r, angle, entry):
    name, effect = entry
    return add(id_, name, "Small", r, angle, [effect])


def angle_of(i):
    return 90 - 40 * i


# The inner ring first, so spokes and clusters can link to it.
ring_ids = []
for k, entry in enumerate(RING):
    a = 90 - 20 * k
    rid = f"ring_{k}"
    if len(entry) == 3:
        add(rid, entry[0], "Notable", R_RING, a, entry[2])
    else:
        small(rid, R_RING, a, entry)
    ring_ids.append(rid)
for k in range(len(ring_ids)):
    link(ring_ids[k], ring_ids[(k + 1) % len(ring_ids)])

for i, c in enumerate(CLASSES):
    t = CLASS_TREES[c]
    a = angle_of(i)
    start = add(f"{c}_start", NAMES[c], "Start", R_START, a, [], cls=c)
    # The spoke inward to the ring.
    prev = start
    for k, (r, entry) in enumerate(zip([360, 290, 225], t["spoke"])):
        prev_new = small(f"{c}_spoke_{k}", r, a, entry)
        link(prev, prev_new)
        prev = prev_new
    link(prev, ring_ids[2 * i])
    # Out from the start, then the fork.
    s1 = small(f"{c}_root", 500, a, SMALL["might"] if c not in ("paladin", "warrior") else SMALL["vigor"])
    link(start, s1)
    ends = []
    for side, sign in (("p", 1), ("n", -1)):
        smalls, (nname, neffects) = t[side]
        prev = s1
        for k, (r, d) in enumerate([(565, 6), (630, 8), (695, 9)]):
            # The middle small of each branch is a utility node: the way to a notable costs
            # something.
            entry = smalls[k] if k != 1 else UTILITY[(2 * i + (side == "n")) % len(UTILITY)]
            nid = small(f"{c}_{side}_{k}", r, a + sign * d, entry)
            link(prev, nid)
            prev = nid
        nid = add(f"{c}_{side}_notable", nname, "Notable", 770, a + sign * 9, neffects)
        link(prev, nid)
        ends.append(nid)
    dname, deffects = t["deep"]
    deep = add(f"{c}_deep", dname, "Notable", 850, a, deffects)
    for e in ends:
        link(e, deep)
    # The two ways sideways to the neighbouring clusters.
    for side, sign in (("p", 1), ("n", -1)):
        sid = small(f"{c}_side_{side}", 455, a + sign * 7, SMALL["swift"] if side == "p" else SMALL["vigor"])
        link(start, sid)

for i, cl in enumerate(CLUSTERS):
    a = angle_of(i) - 20  # between class i and i+1
    k = cl["key"]
    c_prev, c_next = CLASSES[i], CLASSES[(i + 1) % len(CLASSES)]
    e1 = small(f"{k}_entry_0", 470, a + 6, cl["entries"][0])
    e2 = small(f"{k}_entry_1", 470, a - 6, cl["entries"][1])
    link(f"{c_prev}_side_n", e1)
    link(f"{c_next}_side_p", e2)
    inner = small(f"{k}_inner", 400, a, cl["inner"])
    way_in = small(f"{k}_way_in", 300, a, cl["way_in"])
    link(e1, inner)
    link(e2, inner)
    link(inner, way_in)
    link(way_in, ring_ids[2 * i + 1])
    core_name, core_effects = cl["core"]
    core = add(f"{k}_notable", core_name, "Notable", 520, a, core_effects)
    link(e1, core)
    link(e2, core)
    outer = small(f"{k}_outer", 590, a, cl["outer"])
    link(core, outer)
    steer_name, steer_effects = cl["steer"]
    steer = add(f"{k}_lore", steer_name, "Small", 660, a, steer_effects)
    link(outer, steer)
    tip_name, tip_kind, tip_effects = cl["tip"]
    tip_id = f"{k}_keystone" if tip_kind == "Keystone" else f"{k}_tip"
    tip = add(tip_id, tip_name, tip_kind, 750, a, tip_effects)
    link(steer, tip)
    # The neighbours' own trees reach in here too, so every class can walk into the others'.
    link(f"{c_prev}_n_1", outer)
    link(f"{c_next}_p_1", outer)
    if "extra" in cl:
        x_name, x_effects = cl["extra"]
        x = add(f"{k}_extra", x_name, "Notable", 600, a + 9, x_effects)
        link(outer, x)


def ron_node(n):
    parts = [f'id: "{n["id"]}"', f'name: "{n["name"]}"', f'kind: {n["kind"]}']
    if n["cls"]:
        parts.append(f'class: Some("{n["cls"]}")')
    parts.append(f'pos: ({n["pos"][0]}, {n["pos"][1]})')
    if n["links"]:
        parts.append("links: [" + ", ".join(f'"{l}"' for l in n["links"]) + "]")
    if n["effects"]:
        parts.append("effects: [" + ", ".join(n["effects"]) + "]")
    return "        (" + ", ".join(parts) + "),"


HEADER = """// The talent web: one Path of Exile-style web shared by every class (docs/TALENT-WEB.md).
//
// Each class's start node sits on a ring; its own tree grows outward, a spoke runs inward to the
// inner ring that joins every class, and between each pair of neighbouring classes lies a shared
// cluster with a notable, a "lore" node that steers card offers, and a keystone or big notable
// on the rim. Positions are web units (x right, y down); the Talents screen pans and zooms them.
//
// points: one point per talent level, up to `max`. Renown from one level to the next is
// `first` + `step` x the level. Renown comes from runs (together_sim::renown_for_run).
//
// Effects (together_sim::TalentEffect): Stat(stat, amount), Tagged(tag, bonus, amount),
// Hybrid(tag, tag, bonus, amount), Pair(tag, tag, amount), CritBurn(tag, share),
// ChillFreeze(chance), FrozenCrit(chance), Thermal(amount), Execute(amount), CritStreak(tag),
// Leech(share), KillHaste(amount), OfferWeight(tag, amount), AugmentWeight(augment, amount),
// Unlock(augment, [tags]), Rerolls(n), Gold(n), Keystone(name). Bonuses: Damage, Crit,
// CritDamage, Cooldown, Area, Duration, Range.
//
// First laid out by scripts/talent_web.py; this file is the source of truth now.
(
    points: (max: 20, first: 100, step: 30),
    nodes: [
"""

here = os.path.dirname(os.path.abspath(__file__))
out = os.path.join(here, "..", "content", "talents.ron")
with open(out, "w", encoding="utf-8", newline="\n") as f:
    f.write(HEADER)
    for n in nodes:
        f.write(ron_node(n) + "\n")
    f.write("    ],\n)\n")
print(f"{len(nodes)} nodes written to {os.path.normpath(out)}")
