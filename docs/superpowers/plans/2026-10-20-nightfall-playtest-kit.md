# Nightfall sub-project 8: the playtest kit and polish

The spec's seven sub-projects are built (5b, 6 and 7 on 2026-10-19/20). The user will playtest them by hand. This
sub-project makes that playtesting quicker and fairer and clears what earlier reviews left: the spec's own "later"
items (§12), its open questions as toggles (§13; the user's rule: an undecided design choice goes behind a setting
so both can be played), a few review-list items with an obvious answer, and the deferred minors in the ledgers.

No new game design. Every choice a task makes that a player would notice goes on the review list.

Spec: docs/superpowers/specs/2026-10-16-nightfall-design.md (§11 settings, §12 testing, §13 open questions).
Ledgers with deferred minors: `.superpowers/sdd/*/progress.md`, the per-task and final reviews there.

## Rules for every task
- The code as built is the reference; this plan names goals, not line anchors. Find the real code first.
- Tests for every behaviour (unit tests in `nightfall-sim`, pure helpers in the client); no live game windows
  (the user tests by hand: each report ends with "Try by hand" lines).
- Items and gold never copied or lost; co-op: the host decides, clients mirror.
- Keep CRLF files CRLF; rustfmt only files you touch; clippy clean on touched files.

### Task 1: The playtest settings, in the inn
A "Playtest" panel in the inn (a keeper or a button on the inn bar; implementer's choice, on the review list)
that sets, for raids this player hosts or plays solo, the rules the spec leaves open or the user will want to vary:
- `death_loss`: `everything`, `everything_one_recovery` (default), `found_only` (§13).
- Rival kits: simplified or full (§13; the `cutthroats` party already has full kits: make "full" give every rolled
  party full kits of its kind's class).
- Rivals: on (rolled) / off / always one party.
- Signature monsters: rolled / all / none.
- The night clock's speed (×1, ×2, ×4) so a whole night can be seen in a sitting.
- Weather: rolled / clear / mist / rain.
Saved in `benilla-config/` (as Survivors' dev panel saves `together-dev-run.txt`), applied by the host to the
raid's rules and roll at start; a joined player's panel says the host's settings apply. The env switches that
already exist (`NIGHTFALL_WEATHER`, `NIGHTFALL_MONSTERS`, `NIGHTFALL_RIVALS`, ...) still win over the panel.
Pure helpers for "settings → rules/roll" with tests; a test that the default settings change nothing.

### Task 2: Autoplay that sneaks and loots (§12)
`NIGHTFALL_AUTOPLAY` today walks the map and fights. Teach it to sneak when it is a rogue near an unaware enemy,
search containers and bodies on its way, pick up up to its weight, and extract at the nearest open exit once its
bags are half full or the night is late. Its decisions are a pure function of what the hero knows, tested.
The co-op autoplay pair must still start raids.

### Task 3: The showcase (§12)
`NIGHTFALL_SHOWCASE=1` puts one of each enemy kind and each signature monster near the start (out of the entry's
50 yd), a rival party at the nearest point of interest, and a flock of ravens, so each can be seen in one raid.
Positions measured on the bake (a data test that each spot stands). Off by default; inert when unset.

### Task 4: Review-list fixes with an obvious answer
- A sale of a rare-or-better stack asks to confirm (review list 32/38); common stacks sell at once.
- The raid board's Ready button keeps its entity across a rebuild (5b deferred minor: a press could be lost).
- Co-op heroes in the inn already stand apart; check the join while someone stands at the spawn ring.

### Task 5: The deferred minors sweep
Collect every deferred minor in the ledgers and reviews of sub-projects 5b, 6 and 7 that is still true in the
code (read them all, check each against HEAD), fix those that are small and safe, and list the rest with a reason.
Do not touch engine crates for this (e.g. no `NameCache::remove`); say so where that is the fix.

### Task 6: Docs, the full check
CLAUDE.md's Nightfall block: the playtest panel, the showcase, autoplay's new behaviour. The spec's status line
and §12 ("later" items done), §13 (the open questions are now settings). The full test set and a release build.
One autoplay co-op pair run (logs only). The merged "Try by hand" checklist in the report.
