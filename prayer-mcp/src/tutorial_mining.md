# Tutorial: Mining Scripts

This guide explains how to write mining scripts in PrayerLang.

## How mining works

The `mine` command targets a resource at the current location and fills your
cargo hold. **Mining always deposits multiple units per operation** — the exact
amount depends on the resource node and your ship's yield. You cannot control
how many units land in cargo per `mine` call; plan your loop condition around
accumulated totals, not individual yields.

After mining, cargo stays on the ship until you explicitly `stash` it (transfer
to station storage) or fly somewhere that accepts it.

## Basic mining loop

Mine until you have collected at least 50 iron ore, stashing after each run:

```prayer
until MINED(iron_ore) >= 50 {
  mine iron_ore;
  stash;
}
```

`MINED(item_id)` returns the total units of that item stashed so far this
session. The loop keeps going as long as the condition is *false*, so
`MINED(iron_ore) >= 50` exits once the goal is met.

## Mining as a skill

Wrap a mining loop in a reusable skill with a parameter for the target item:

```prayer
skill mine_and_stash(item: item_id) {
  until MINED($item) >= 50 {
    mine $item;
    stash;
  }
}
```

Call it from another script with:

```prayer
mine_and_stash iron_ore;
```