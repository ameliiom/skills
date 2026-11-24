# Skills

Skill trees for [Horizon](https://playhorizon.dev) (Minecraft: Java Edition 26.3 in Rust), in Minecraft's own look. Play as you always do: fighting, mining, farming, exploring and building each earn experience in their own tree. Friends near you earn half of what you earn, and you keep all of yours. Spend the points on permanent perks. English and Arabic; Arabic mirrors the screen.

## How it plays
- **Press `K`** to open the skills screen. It is built like the advancements screen: one tab per tree, framed skills joined by lines, and the tree's level, experience bar and points at the top.
- **Gold frame**: unlocked. **Light frame**: next in line (one click unlocks it when you have the points). **Dark**: needs the skill before it.
- **Hover a skill** to see what it gives, as the game says it on items (`+2 Max Health`, `+10% Block Break Speed`), and what it costs.
- **Every level is one point** for that tree. When a tree levels up, a toast appears in the game's toast corner. There is no chat line and no command.
- **Shared with friends**: whatever experience you earn, every other player within 32 blocks in the same dimension earns half of it too. The range and the share are in the mod's settings.
- Perks are attribute modifiers, rebuilt from your progress whenever you join, respawn or unlock. Progress is saved per player in the world (`players/data/<uuid>.skills.json`).

## The five trees
| Tree | Earns from | Perks |
|---|---|---|
| Combat | kills, damage dealt | attack damage, max health, attack speed, knockback resistance, sweeping, armour |
| Mining | pickaxe and shovel blocks, more for ores | block break speed, mining underwater, efficiency, heat proof, luck |
| Farming | fully grown crops, melons, pumpkins, breeding, fishing | max health, luck, entity reach |
| Exploration | walking, sprinting, swimming, climbing, riding, flying | speed, safe fall, feather fall, jump, step height, oxygen, swimming |
| Building | placing blocks | block reach, crouch speed, entity reach, safe fall |

## Add your own trees
Trees are data. Any data pack or mod can add a tree, add skills to an existing one, or replace one:

```json
// data/<namespace>/skills/<tree>.json
{
  "title": "mypack.tree.fishing", "icon": "minecraft:fishing_rod", "background": "minecraft:block/prismarine",
  "order": 6, "curve": { "base": 20, "step": 10 },
  "sources": [
    { "stat": "minecraft:custom", "of": "minecraft:fish_caught", "xp": 5 },
    { "stat": "minecraft:mined", "of": "#minecraft:logs", "xp": 1 },
    { "block": "#minecraft:crops", "state": { "age": "7" }, "xp": 2 },
    { "block": "*", "on": "placed", "xp": 0.5 }
  ],
  "nodes": [
    { "id": "lucky", "title": "mypack.node.lucky", "icon": "minecraft:cod", "at": [0, 0], "cost": 1,
      "perks": [{ "attribute": "minecraft:luck", "amount": 1, "operation": "add_value" }] },
    { "id": "luckier", "icon": "minecraft:salmon", "at": [1, 0], "cost": 2, "needs": ["lucky"],
      "perks": [{ "attribute": "minecraft:luck", "amount": 1, "operation": "add_value" }] }
  ]
}
```

- **Sources**: a statistic (`stat` type, and `of` an id, a `#tag` or `*`), worth `xp` per count; or a block broken (or placed, with `"on": "placed"`) whose state has every listed property.
- **Skills**: a place on the grid (`at`, from the reading start), a cost, the skills they need, and perks on any attribute of the game or a mod.
- **Add to a tree**: `{"extends": "skills:mining", "nodes": [...], "sources": [...]}`.
- A skill with an unknown attribute or a missing `needs` is skipped and named in the log; the rest loads.

## Install
1. Copy `skills.hmod` into `%APPDATA%\horizon\horizon-native-mods\`.
2. Add the line `mod.skills:true` to `%APPDATA%\horizon\horizon-mods.txt`.
3. Build: in the launcher's Mods page press **Build**, or run `horizon-launcher native-build --instance horizon --trust-native-code`. A native mod runs with your rights: install it only from a source you trust.

The server and every player need the mod. On a server without it, the screen says so.

Built for Horizon Nightly `f037744` (2026-11-21).

## Build
```powershell
cargo horizon-mod build
```

## Licence
MIT, see [LICENSE](LICENSE). The default trees' balance follows the idea of Default Skill Trees (CC0). Not affiliated with the Horizon project.
