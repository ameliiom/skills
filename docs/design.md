# Skills: design

Earn skill experience by playing, share it with friends nearby, and spend points in skill trees on
permanent perks. One screen in the advancements' own look, one toast on a new level, no commands.

## 1. Domain model (one owner per fact)

| Concept | Owner | Stored? |
|---|---|---|
| **Tree**: a skill such as Mining: icon, title, background, level curve, nodes, sources | data pack JSON (`data/<ns>/skills/<tree>.json`) | data |
| **Node**: one perk on a tree: icon, place on the grid, cost, the nodes it needs, its perks | the tree file, or an `extends` file | data |
| **Perk**: what a node gives: an attribute modifier (any attribute, any mod) | the node | data |
| **Source**: an action that earns a tree experience (a statistic, or a broken block in a given state) | the tree file, or an `extends` file | data |
| **Progress**: per player, per tree: total experience earned, nodes unlocked | the server, `players/data/<uuid>.skills.json` | yes, the only stored state |
| Level, points earned, points left | derived from progress and the curve | never stored |
| Attribute modifiers on the player | derived from the unlocked nodes, reconciled on join, on unlock and on reload | never the source |

`Progress` is the single source of truth. Everything a player sees and every modifier on them is
computed from it and the loaded trees (desired-state reconciliation: a tree edited in a pack
corrects every player on the next reload, with no migration).

## 2. Earning experience

- **One mechanism, statistics.** Vanilla counts almost every action as a statistic (`minecraft:mined`
  `minecraft:stone`, `minecraft:killed` `minecraft:zombie`, `minecraft:custom`
  `minecraft:fish_caught`, `minecraft:custom` `minecraft:walk_one_cm`, `minecraft:crafted` …). One
  hook (`player_stat_award`) turns an award into tree experience through the sources the trees
  declare, so a mod's own blocks, mobs and items earn experience with no code.
- **Block state sources** (`player_destroyed_block`) cover what a statistic can't tell: a crop only
  when fully grown (`"state": {"age": "7"}`), so planting and breaking seeds earns nothing.
- Sources name an id or a tag (`#minecraft:logs`, `#c:ores`).
- **Shared with friends.** When you earn tree experience, every other player in the same dimension
  within `share_radius` blocks (32) also earns `share_fraction` of it (50%) on the same tree. You
  keep all of yours; nothing is split away. Server settings, no Team dependency needed (everyone on
  the server plays together, as in Team).

## 3. Levels and points

- Level *n* needs `base + step × n` more experience (defaults 50 and 25, per tree in its file).
- Every level gives one point for that tree. A tree's levels stop when its points would cover every
  node (no dead levels).
- A node is open when every node it needs is unlocked and the tree has enough points left. One
  click unlocks it. Unlocking is permanent.

## 4. Perks

- A perk is an attribute modifier: `{"attribute": "minecraft:block_break_speed", "amount": 0.1,
  "operation": "add_multiplied_base"}`. Any attribute the game or a mod registers works.
- Modifier ids are `skills:<tree namespace>/<tree path>/<node>/<n>` (`n` = the perk's place in the node) so they never collide and are found again on reload.
- Applied as transient modifiers rebuilt from progress (join, respawn, unlock, reload), so a deleted
  node or a changed amount never leaves a stale modifier saved on the player.

## 5. The screen (key `K`)

Built in the advancements screen's own look, from the game's sprites:

- The advancements window frame (`gui/advancements/window.png`, drawn nine-slice by its 9/18/9/9
  border) filling the screen but for a 16-pixel margin, so more of a tree shows than vanilla's fixed
  252×140 window. One tab per tree on top (the tree's icon, the `tab_above_*` sprites), from the
  reading start. When the trees don't fit, the last tab (a spectral arrow, "More skills") turns to
  the next page of tabs and wraps back to the first, so every tree stays one or two clicks away
  with no space taken outside the window.
- The tree's background tiled from a block texture (stone for Mining, dirt for Farming…), dragged to
  pan like the advancements map.
- Each node in the advancement frames: **gold frame** = unlocked, **plain frame** = open (enough
  points, needs met), **dark frame and darkened icon** = locked. Connections drawn as the advancement
  lines (white over black).
- At the top of the window: the tree's level as the green experience number, the experience bar
  sprite filled to the next level, and points left.
- Hovering a node: the advancement title box with its name, then the perk lines in the attribute
  tooltip's blue (`+10% Mining Speed`), then its cost.
- One click on an open node unlocks it (the advancement "challenge complete" sound, muted with the
  game's sound settings).
- Arabic mirrors the screen: tabs from the right, trees grow right to left.

## 6. Toast

On a new level: a toast in the advancement toast's frame, the tree's icon, "Mining 5", "+1 point".
Drawn by Skills at the game's toast corner (top right in every language, as vanilla's own toasts, over whatever is there for its 5 seconds); Arabic mirrors its inside. Several stack downward. No
chat line.

## 7. Data format

```json
// data/skills/skills/mining.json
{
  "title": "skills.tree.mining",
  "icon": "minecraft:iron_pickaxe",
  "background": "minecraft:block/stone",
  "order": 2,
  "curve": { "base": 50, "step": 25 },
  "sources": [
    { "stat": "minecraft:mined", "of": "#minecraft:mineable/pickaxe", "xp": 1 },
    { "stat": "minecraft:mined", "of": "#c:ores", "xp": 8 }
  ],
  "nodes": [
    { "id": "haste_1", "icon": "minecraft:golden_pickaxe", "at": [0, 0], "cost": 1,
      "perks": [{ "attribute": "minecraft:block_break_speed", "amount": 0.1, "operation": "add_multiplied_base" }] },
    { "id": "haste_2", "icon": "minecraft:diamond_pickaxe", "at": [1, 0], "cost": 2, "needs": ["haste_1"],
      "perks": [{ "attribute": "minecraft:block_break_speed", "amount": 0.15, "operation": "add_multiplied_base" }] }
  ]
}
```

- Another pack or mod adds to a tree with `{"extends": "skills:mining", "nodes": [...], "sources": [...]}`
  in its own namespace; a pack replaces a tree by overriding the same file, as data packs do.
- A block state source: `{ "block": "#minecraft:crops", "state": { "age": "7" }, "xp": 2 }`.
- Titles and node names are translation keys (`skills.node.mining.haste_1`); a missing key shows
  the node id.
- Invalid entries (unknown attribute, unknown `needs`, a cycle, a duplicate id) are skipped with one
  log line naming the file; the rest of the tree loads.

## 8. Default trees

Five trees in an always-on data pack bundled in the mod (`assets/skills/packs/defaults`, registered
with `add_pack_finders` at the bottom of the pack order, hidden), so any world or mod pack overrides or
extends them. Their balance follows the idea of Default Skill Trees (CC0):

| Tree | Earns from | Perks |
|---|---|---|
| Combat | kills (`minecraft:killed`), damage dealt | attack damage, attack speed, max health, armour, armour toughness, knockback resistance, sweeping |
| Mining | pickaxe and shovel blocks, more for ores | mining speed, mining efficiency, submerged mining, burning time, luck |
| Farming | fully grown crops, melons, pumpkins, breeding, fishing | max health, luck, entity reach |
| Exploration | walking, sprinting, swimming, climbing, riding, flying | speed, safe fall distance, fall damage, jump, step height, oxygen, water movement |
| Building | placed blocks (a `placed` block source) | block reach, sneaking speed, entity reach, safe fall distance |

Each tree: 7–11 nodes in two or three branches, 8–40 points to fill (a test holds every shipped tree to that range), curve 20 + 10 × level.

## 9. Network

| Payload | Way | When |
|---|---|---|
| `skills:trees` | server → client | on join and after every data reload: the loaded trees (layout, icons, titles, perks, curve) |
| `skills:progress` | server → client | on join and on every change: that player's experience and unlocked nodes per tree |
| `skills:unlock` | client → server | a click on an open node; the server checks and answers with `skills:progress` |

A payload string holds 4096 bytes, so the JSON travels as a list of parts. Trees and progress are
sent from the server tick, retried until the client takes them (a join's first moments can't carry
payloads), and perks are reconciled on the tick after a join or respawn for the same reason.

Optional payloads: on a server without Skills the key shows an empty window with "Skills is not on this
server". A client without Skills still plays; its player earns nothing.

## 10. Settings

- Server: `share_radius` (0–128, 32), `share_fraction` (0–1, 0.5).
- Client: `toasts` (on).

## 11. Not in this version

- Resetting a tree (respec).
- Perks that are not attributes (bonus crop drops, auto-smelt): the perk format is a tagged list so
  they add later as new kinds without changing files.
- Experience for blocks a player placed and broke again (ores): counted as vanilla counts them.
