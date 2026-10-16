# Nightfall sub-project 2 (World knowledge): rulings

Decisions the spec (`docs/superpowers/specs/2026-10-16-nightfall-design.md`) does not settle, made
for `2026-10-17-nightfall-world-knowledge.md`. One line each: what; why; cost if wrong.

## The height map

- Terrain-only columns keep one `i16` height in eighths of a yard; columns with anything more keep their whole span list (sorted, binary-searched); why: Duskwood's rectangle is 17 M columns, so 34 MB instead of ~140 MB; cost if wrong: a codec and builder change, no API change.
- The terrain is a solid shell 2 yd deep (`TERRAIN_DEPTH`), not solid to the bottom of the world; why: a crypt or mine under a hillside keeps its air (spec §4.1 asks for crypts and caves); cost if wrong: a ray steep enough to pass under a 2 yd shell between two column samples — none at eye height has been found.
- Solids closer than 0.2 yd merge (`MERGE_GAP`); headroom 2 yd (`HEADROOM`); a step 0.7 yd (`MAX_STEP`, about 55°); why: a hero's height and WoW's walkable slope; cost if wrong: constants to tune, and the cache's `BAKE_VERSION` to bump.
- A column samples terrain at its centre only; why: 17 M fan-triangle lookups already cost about a second; cost if wrong: up to a quarter-yard height error on steep slopes, which the step test absorbs.
- A column holds at most 8 floors (`MAX_FLOORS`); why: a fixed array keeps the path search allocation-free; cost if wrong: a stack of thin slabs loses its upper floors.
- Impassable chunks are blocked whole for walking, never for sight; why: the engine fences them for movers only (its LOS is wall-blind to them); simpler than one-way fences; cost if wrong: enemies cannot enter a flagged chunk a hero can walk inside, which is rare (zone edges).
- Columns outside the bounds polygon are blocked; why: no enemy path leaves the run; cost if wrong: none found.
- Water is ignored; why: not in §4; trails and water come with sub-project 6; cost if wrong: enemies wade across a lake bed.

## Sight

- An end's own column ignores a span that holds that end; why: a hero pressed to a wall shares its half-yard column, and must not go blind and invisible; cost if wrong: a wall wholly inside one column (thinner than ~0.5 yd and not straddling an edge) can be seen through by someone touching it.
- Eyes at 1.7 yd, the point seen at the chest, 1.2 yd (`EYE_HEIGHT`, `CHEST_HEIGHT`); why: §4.2 says "eye height to eye height (or to chest height)"; cost if wrong: two constants.
- Sight range is the fog's end (40 yd) for heroes too, as §5.1 says for enemies; why: §4.3 says "in the fog distance"; cost if wrong: one line.
- Without a map (a client's mirror, a bake that failed) sight is distance only; why: nothing is known to stand between; cost if wrong: a failed bake shows enemies through walls, which the failure line already explains.
- The client does not get the `Concealed` marker now: the seen set is computed on the host, sent per player in `State`, and kept in `Sim::seen` (with `NIGHTFALL_TRACE_SEEN` to see it); why: spec §2 puts "hidden enemies kept out of the UI" in sub-project 3, and the marker touches targeting, nameplates and the minimap; cost if wrong: one client task in sub-project 3 (the data is ready).
- The host takes the hero's height from the client's pose (`HeroPose::z`), snapped to a floor within 1 yd, kept as reported mid-jump; why: only the client knows which storey its hero is on; cost if wrong: a cheating client could lie about its height (friends' co-op, spec §8.3).

## Paths

- One column of clearance (a unit stands only where all eight neighbours are steppable); why: units walk beside walls, not half inside them; cost if wrong: doorways narrower than 1.5 yd are closed to enemies.
- No corner cutting on diagonals; integer costs 10 and 14 with an octile heuristic; ties go deeper; why: deterministic paths that stay on their line over open ground; cost if wrong: none.
- Budgets: 40 000 floors for a patrol leg or a post, 4 000 for a wander walk; at most 4 searches a step (`paths_per_step` in `rules.ron`), handed round the enemies; why: a sealed goal costs milliseconds, never a hitch; cost if wrong: a very long patrol leg may need its points closer together (the warning names it).
- Only patrol legs and posts are cached (by snapped start and goal floor, cleared at 1024); why: they repeat, wander walks do not; cost if wrong: a little memory or a little search.
- Smoothing joins points into straight legs of at most 40 columns, each checked with `walk_line` every quarter yard; why: it bounds the smoothing's work; cost if wrong: a few more waypoints on long open stretches.
- A patrol point no path reaches is skipped for the rest of the run with one warning; a sentry retries its post every 5 s; a wanderer warns after 10 failed walks in a row; why: content mistakes show in the log and never freeze the host; cost if wrong: a patrol cut short until the content is fixed.

## Enemies

- Three behaviours now: `Sentry`, `Patrol` (loops back to its first point) and `Wander`; sleepers, trackers, casters and elites wait for senses (3), combat (4) and the night clock (6); why: the playable result needs walking, not noticing; cost if wrong: none (the `Brain` is where they plug in).
- Patrols loop rather than walk back and forth; why: the simpler route; cost if wrong: a `pingpong` flag on `Patrol`.
- A group of `count` sentries stands 1.5 yd apart around its post; a patrol group starts spread over its route's points; a wander group starts at random spots in its area; why: no stacking; cost if wrong: content layout only.
- A spawn's floor is the one nearest its optional `z`, else nearest the terrain; why: upper storeys are opt-in, rooftops never by accident; cost if wrong: one more field.
- Enemy numbers are placeholders near vanilla level 25 to 30 (900 to 1100 hp, walk 1.8 to 3 yd/s); combat (sub-project 4) tunes them; cost if wrong: content edits.
- Six kinds from the survivors content's Duskwood list (skeletal warrior, Nightbane worgen and shadow weaver, venom web spider, black ravager, fetid corpse), with its display ids; why: known to have models in the trimmed data; cost if wrong: content edits.
- Every enemy group sits near the first entry, each marked `measure:` with the place it belongs (Raven Hill, Brightwood Grove, Twilight Grove, the Rotting Orchard); why: no coordinates in the repo beyond the entry, and the brief says to place relative to it; cost if wrong: a content pass after measuring in the game, helped by the `buildings` example.
- Randomness is a per-enemy xorshift seeded from its id; why: the same run plays the same, as the rest of the sim does; cost if wrong: sub-project 6's per-raid roll adds a run seed.
- Enemy kinds are creature entries `index + 1` in the client's name cache, hostile faction 14; why: the survivors client's pattern; cost if wrong: none.

## The bake

- A new crate, `nightfall-bake`, owns everything that reads WoW files; `nightfall-sim` only has the map type and its builder; why: the sim stays pure and tests with hand-made maps (brief); cost if wrong: none.
- Buildings use the WMO walking-collision set (DETAIL faces skipped) for both walking and sight, not the camera set; why: one span list per column, and the two sets differ only in small overhangs; cost if wrong: a forge pipe a camera stops on does not stop sight.
- WMO interior doodads (furniture from MODD sets) are not baked; why: no loader for them in `benilla-formats`' collision API, and the engine welds them separately; cost if wrong: enemies walk through tables indoors.
- A doodad's collision hull is filled solid per column (lowest to highest point); a building's spans merge only where they touch; why: a hull is a closed solid, a building has rooms; cost if wrong: a hollow doodad (an open crate) is solid.
- Placements are taken when their origin is within 250 yd (buildings) or 30 yd (doodads) of the bounds; why: geometry reaching in from outside; cost if wrong: a huge building just outside is missed.
- The map's directory comes from `Map.dbc`; why: no hard-coded "Azeroth"; cost if wrong: none.
- The bake runs on one thread with no parallelism; why: under-a-minute looks reachable (17 M terrain lookups ~1 s, hulls a few s), and Task 7 times it; cost if wrong: a rayon pass over tiles if the example shows over 60 s.
- The bake region is the bounds' bounding rectangle, and the bounds stay the foundation's rectangle; tracing Duskwood's real edge waits; why: no zone outline in the repo, and the brief only asks to bake the bounds region; cost if wrong: a bake about twice as large as needed, and enemies may roam the rectangle's corners in other zones (none are placed there).
- The spec §9.2 survey (which points of interest have interiors, which are reachable) is a content pass in the game with the `buildings` example and `NIGHTFALL_TRACE_POS`, not a task here; why: it needs a person walking the zone; cost if wrong: one exploratory session.
- Props: the bake takes a prop list and its hash covers it, but `props.ron` and the client spawning its models wait for the sub-project that first places cover; why: an unseen baked prop would be an invisible wall; cost if wrong: one content file and one client task.

## The cache

- The data fingerprint is every MPQ's lower-case name, size and modified time, not its bytes; why: hashing gigabytes at every launch is slower than baking; cost if wrong: a file replaced with one of the same size and time keeps a stale cache (delete `benilla-config/nightfall/`).
- The cache file is the map's own bytes deflated at the fast level, written through a temporary file and renamed; older files for the same map key are deleted; why: about 56 MB raw shrinks several times, and a crash never leaves half a file under the real name; cost if wrong: disk space.
- A cache that does not decode (cut short, foreign, another version) is baked again, silently; why: the cache is disposable; cost if wrong: a slow launch, never a wrong map.

## The client

- Every player bakes when Nightfall's menu first shows, not only a host; why: the menu does not know yet who will host, and the next host then starts at once; cost if wrong: a joiner's CPU for under a minute, once.
- A run does not wait for the bake: its enemies appear when the map is ready, and a failed bake plays without enemies and says so in the line; why: the menu and loading screen usually cover the bake, and a run is never blocked; cost if wrong: a first-launch solo run may begin without enemies for some seconds.
- The progress line is a plain text node at the bottom centre, above the loading screen (`GlobalZIndex(1001)` over its 1000), shown on the menu and in the run; why: the loading screen has no progress-line hook, and the brief keeps engine changes minimal; cost if wrong: a styling change.
- Each client's snapshot holds only the enemies within the view distance (120 yd) of its own hero; why: 120 enemies at 15 Hz would be ~60 KB/s per client; cost if wrong: an enemy beyond 120 yd is not on a client's map (sub-project 3's minimap may want more).
- Enemies are drawn within the view distance of the local hero and kept until 15 yd past it; why: no blinking at the edge, and no hundred creatures animating out of sight on the host; cost if wrong: none.
- An enemy is drawn on the client's own terrain height when the host's floor is within 1 yd of it, else at the host's floor; why: the client's terrain is exact, the bake's columns are not, and upper storeys need the host's height; cost if wrong: an enemy on a low porch may sink to the ground beside it.
- Walking uses `FORWARD | WALK_MODE` up to half a yard a second over the kind's walk speed; why: the walk animation rather than a slow run; cost if wrong: a flag.
- The protocol goes to 2 (`State` gains `seen`); why: the message changed shape; cost if wrong: none.
- Manual runs set `BENILLA_HOME=target/nf-home`; why: the cache's place is known and the user's own state is untouched; cost if wrong: none.
