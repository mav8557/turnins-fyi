#!/usr/bin/env python3
"""
Generates data/quest_recipes.jsonl — recipe data for the crafted items required
by each DoH job's class quests (levels 1-70). One JSON object per line, sorted
by job then turnin_level.

Each crafted hand-in item gets a record with `turnin_level`/`turnin_count` set
to the quest level and required quantity. Any ingredient that is itself
craftable is recursively expanded into its own record (with `turnin_level`/
`turnin_count` set to null, since it isn't directly handed in for a quest) so
the full subcraft tree is present in one flat file.

Gatherer jobs (MIN/BTN/FSH) have class quests too, but their hand-in items are
gathered materials rather than crafted goods, so they produce no output lines.

Sources:
  - https://ffxiv.consolegameswiki.com/wiki/<Job>_Quests ("Quest Hand-in Items"
    tables — transcribed below as QUEST_ITEMS, with item IDs resolved by name)
  - TeamCraft recipes.json (recipe ingredients by job)
  - XIVAPI (item names)

Run from the data/ directory: python3 gen_quest_recipes.py
"""
import json
import time
import sys
import urllib.request

JOBS = {
    8:  ("Carpenter",     "CRP"),
    9:  ("Blacksmith",    "BSM"),
    10: ("Armorer",       "ARM"),
    11: ("Goldsmith",     "GSM"),
    12: ("Leatherworker", "LTW"),
    13: ("Weaver",        "WVR"),
    14: ("Alchemist",     "ALC"),
    15: ("Culinarian",    "CUL"),
}
ABBR_TO_JOB_ID = {abbr: job_id for job_id, (_, abbr) in JOBS.items()}

TEAMCRAFT_RECIPES_URL = (
    "https://raw.githubusercontent.com/ffxiv-teamcraft/ffxiv-teamcraft"
    "/master/libs/data/src/lib/json/recipes.json"
)

# Quest hand-in items transcribed from the "Quest Hand-in Items" table on each
# job's consolegameswiki page (e.g. https://ffxiv.consolegameswiki.com/wiki/Carpenter_Quests).
# Annotations like "(melded with ... materia)" and "(normal quality acceptable)"
# describe how the item must be turned in, not the item itself, and are dropped
# here — only the base crafted item name/quantity matters for the recipe tree.
# Each entry is (turnin_level, [(item_id, count, item_name), ...]).
QUEST_ITEMS = {
    "CRP": [
        (1,  [(5361, 1, "Maple Lumber")]),
        (5,  [(2219, 3, "Square Maple Shield")]),
        (10, [(5364, 12, "Ash Lumber")]),
        (15, [(1823, 1, "Feathered Harpoon"), (1895, 1, "Ash Shortbow")]),
        (20, [(1827, 1, "Iron Lance")]),
        (25, [(5371, 1, "Walnut Lumber")]),
        (30, [(2015, 1, "Walnut Cane")]),
        (35, [(1917, 1, "Oak Longbow")]),
        (40, [(1925, 1, "Oak Composite Bow")]),
        (45, [(1862, 1, "Cobalt Halberd"), (2039, 1, "Jade Crook"), (1937, 1, "Yew Longbow")]),
        (50, [(5376, 1, "Rosewood Lumber"), (1942, 1, "Crab Bow")]),
        (53, [(10611, 1, "Holy Cedar Composite Bow")]),
        (55, [(10636, 1, "Dark Chestnut Rod")]),
        (58, [(12583, 3, "Birch Lumber")]),
        (60, [(10609, 1, "Adamantite Trident")]),
        (63, [(17878, 1, "Lavish Dressing Case")]),
        (65, [(17880, 1, "Stylish Pipe Box")]),
        (68, [(17882, 1, "Ornate Partition")]),
        (70, [(17884, 1, "Teahouse Bench")]),
    ],
    "BSM": [
        (1,  [(5056, 1, "Bronze Ingot")]),
        (5,  [(2341, 3, "Bronze Cross-pein Hammer")]),
        (10, [(5091, 12, "Bronze Rivets")]),
        (15, [(1605, 1, "Bronze Spatha"), (1753, 1, "Spiked Bronze Labrys")]),
        (20, [(2344, 1, "Iron Cross-pein Hammer")]),
        (25, [(5058, 1, "Steel Ingot")]),
        (30, [(2551, 1, "Plumed Iron Hatchet")]),
        (35, [(1778, 1, "Steel Bhuj")]),
        (40, [(2350, 1, "Wrapped Crowsbeak Hammer")]),
        (45, [(1723, 1, "Cobalt Knuckles"), (1796, 1, "Buccaneer's Bardiche"), (1659, 1, "Cobalt Shamshir")]),
        (50, [(1663, 1, "Cobalt Winglet")]),
        (53, [(12528, 3, "Mythrite Rivets")]),
        (55, [(10588, 1, "Titanium Bastard Sword")]),
        (58, [(11877, 1, "Titanium Lump Hammer")]),
        (60, [(10591, 1, "Adamantite Winglet")]),
        (63, [(17886, 1, "Featherlight Shortsword")]),
        (65, [(17888, 1, "Heavy Uchigatana")]),
        (68, [(17890, 1, "Custom-made Hammer")]),
        (70, [(17892, 1, "Examination Uchigatana")]),
    ],
    "ARM": [
        (1,  [(5056, 1, "Bronze Ingot")]),
        (5,  [(2226, 3, "Bronze Hoplon")]),
        (10, [(5071, 12, "Bronze Plate")]),
        (15, [(2680, 1, "Decorated Bronze Barbut"), (2231, 1, "Bronze Buckler")]),
        (20, [(2236, 1, "Iron Hoplon")]),
        (25, [(5058, 1, "Steel Ingot")]),
        (30, [(3096, 1, "Steel Chainmail")]),
        (35, [(2502, 1, "Steel Frypan")]),
        (40, [(3149, 1, "Mythril Cuirass")]),
        (45, [(3861, 1, "Mythril-plated Caligae"), (2830, 1, "Reinforced Mythril Elmo"), (3868, 1, "Mythril Sollerets")]),
        (50, [(3203, 1, "Cobalt Haubergeon")]),
        (53, [(10756, 1, "Titanium Mask of Striking")]),
        (55, [(10721, 1, "Titanium Cuirass of Maiming")]),
        (58, [(10667, 1, "Titanium Hoplon")]),
        (60, [(10682, 1, "Adamantite Lorica of Fending")]),
        (63, [(17894, 1, "Paladin Mail")]),
        (65, [(17896, 1, "Titanium Kote")]),
        (68, [(17898, 1, "Tournament Somen")]),
        (70, [(17900, 1, "Lominsan Hara-ate")]),
    ],
    "GSM": [
        (1,  [(5062, 1, "Copper Ingot")]),
        (5,  [(4305, 3, "Copper Gorget")]),
        (10, [(5086, 12, "Copper Rings")]),
        (15, [(4204, 1, "Fang Earrings"), (4309, 1, "Brass Gorget")]),
        (20, [(2111, 1, "Staghorn Staff")]),
        (25, [(5064, 1, "Silver Ingot")]),
        (30, [(4218, 1, "Malachite Earrings")]),
        (35, [(2078, 1, "Fire Brand")]),
        (40, [(2125, 1, "Horn Staff")]),
        (45, [(2876, 1, "Electrum Circlet (Amber)"), (2878, 1, "Electrum Circlet (Spinel)"), (2872, 1, "Electrum Circlet (Zircon)")]),
        (50, [(4514, 1, "Black Pearl Ring")]),
        (53, [(11068, 1, "Hardsilver Bangle of Casting")]),
        (55, [(12521, 1, "Hardsilver Ingot")]),
        (58, [(12644, 1, "Aurum Regis Cylinder")]),
        (60, [(12108, 1, "Star Sapphire Music Box"), (12109, 1, "Star Ruby Music Box")]),
        (63, [(17902, 1, "Sample Silver Ring")]),
        (65, [(17904, 1, "Precision Spectacles")]),
        (68, [(17906, 1, "Sample Chronometer")]),
        (70, [(17908, 1, "Tribute Orchestrion")]),
    ],
    "LTW": [
        (1,  [(5275, 1, "Leather")]),
        (5,  [(4304, 3, "Leather Choker")]),
        (10, [(5276, 12, "Hard Leather")]),
        (15, [(3772, 1, "Hard Leather Caligae"), (4308, 1, "Hard Leather Choker")]),
        (20, [(3784, 1, "Goatskin Leggings")]),
        (25, [(5280, 1, "Toad Leather")]),
        (30, [(3100, 1, "Toadskin Jacket")]),
        (35, [(3616, 1, "Boarskin Ringbands")]),
        (40, [(3639, 1, "Boarskin Smithy's Gloves")]),
        (45, [(3651, 1, "Fingerless Raptorskin Gloves"), (2270, 1, "Raptorskin Targe"), (4360, 1, "Raptorskin Choker")]),
        (50, [(3206, 1, "Raptorskin Jerkin")]),
        (53, [(12007, 1, "Wyvernskin Workboots")]),
        (55, [(10875, 1, "Dhalmelskin Leggings of Scouting")]),
        (58, [(12024, 1, "Dragonskin Choker")]),
        (60, [(12999, 1, "Chivalric Longcoat of Aiming")]),
        (63, [(17910, 1, "Large Dhalmel Cape")]),
        (65, [(17912, 1, "Grizzly Bear Gloves")]),
        (68, [(17914, 1, "Dashing Dhalmelskin Jacket")]),
        (70, [(17916, 1, "Large Gagana Cape")]),
    ],
    "WVR": [
        (1,  [(5333, 1, "Hempen Yarn")]),
        (5,  [(3274, 3, "Hempen Breeches")]),
        (10, [(5324, 12, "Undyed Hempen Cloth")]),
        (15, [(2670, 1, "Cotton Scarf"), (3322, 1, "Cotton Shepherd's Slops")]),
        (20, [(3039, 1, "Cotton Acton")]),
        (25, [(5326, 1, "Undyed Velveteen")]),
        (30, [(3823, 1, "Velveteen Gaiters")]),
        (35, [(3125, 1, "Linen Shirt")]),
        (40, [(3413, 1, "Woolen Tights")]),
        (45, [(2829, 1, "Woolen Beret"), (3176, 1, "Woolen Gown"), (3429, 1, "Woolen Gaskins")]),
        (50, [(3475, 1, "Patrician's Bottoms"), (2895, 1, "Patrician's Wedge Cap"), (3215, 1, "Patrician's Coatee")]),
        (53, [(11966, 1, "Holy Rainbow Gloves")]),
        (55, [(10881, 1, "Holy Rainbow Hat of Healing")]),
        (58, [(12596, 3, "Crawler Silk")]),
        (60, [(13001, 1, "Chivalric Doublet of Healing")]),
        (63, [(17918, 1, "Elegant Bustle")]),
        (65, [(17920, 1, "Winsome Spring Dress")]),
        (68, [(17922, 1, "Seductive Bustier")]),
        (70, [(17924, 1, "Tennyo Hagoromo")]),
    ],
    "ALC": [
        (1,  [(5487, 1, "Distilled Water")]),
        (5,  [(4564, 3, "Antidote")]),
        (10, [(5515, 12, "Beeswax")]),
        (15, [(4597, 1, "Potion of Intelligence"), (4595, 1, "Potion of Dexterity")]),
        (20, [(2149, 1, "Engraved Hard Leather Grimoire")]),
        (25, [(5522, 1, "Natron")]),
        (30, [(4575, 3, "Weak Blinding Potion")]),
        (35, [(4556, 1, "Hi-Ether")]),
        (40, [(4599, 3, "Hi-Potion of Strength")]),
        (45, [(4607, 1, "Mega-Potion of Intelligence"), (4608, 1, "Mega-Potion of Mind"), (4606, 1, "Mega-Potion of Vitality")]),
        (50, [(1987, 1, "Budding Rosewood Wand")]),
        (53, [(12601, 3, "Enchanted Mythrite Ink")]),
        (55, [(12615, 3, "Grade 1 Intelligence Dissolvent")]),
        (58, [(12622, 1, "Draconian Potion of Strength")]),
        (60, [(10651, 1, "Noble Gold")]),
        (63, [(17926, 1, "Intellectuary")]),
        (65, [(17928, 1, "Twice-fermented Mun-Tuy Juice")]),
        (68, [(17930, 1, "Luminol")]),
        (70, [(17932, 1, "Potent Dissolvent")]),
    ],
    "CUL": [
        (1,  [(4849, 1, "Maple Syrup")]),
        (5,  [(4660, 1, "Grilled Trout")]),
        (10, [(4640, 2, "Grilled Dodo")]),
        (15, [(4642, 1, "Meat Miq'abob")]),
        (20, [(4730, 1, "Dried Plums")]),
        (25, [(4644, 1, "Aldgoat Steak")]),
        (30, [(4645, 1, "Smoked Raptor")]),
        (35, [(4677, 1, "Ratatouille")]),
        (40, [(4712, 1, "Blood Currant Tart"), (4733, 1, "Pastry Fish"), (4749, 1, "Chamomile Tea")]),
        (45, [(4679, 1, "Dzemael Gratin")]),
        (50, [(4647, 1, "Eft Steak"), (4678, 1, "Beef Stew"), (4715, 1, "Trapper's Quiche"), (4713, 1, "Crowned Pie")]),
        (53, [(12842, 1, "Ishgardian Tea"), (12846, 1, "Sohm Al Tart")]),
        (55, [(12849, 1, "Kaiser Roll"), (12862, 1, "Beet Soup"), (12855, 1, "Grilled Sweetfish")]),
        (58, [(12858, 1, "Cockatrice Meatballs")]),
        (60, [(12854, 1, "Morel Salad"), (12860, 1, "Deep-Fried Okeanis"), (12847, 1, "Marron Glace")]),
        (63, [(17934, 1, "Doman Rice Balls")]),
        (65, [(17936, 1, "Doman Udon Broth")]),
        (68, [(17938, 1, "Nigiri-zushi")]),
        (70, [(17940, 1, "Doman Sukiyaki")]),
    ],
}


def fetch_json(url: str, label: str) -> object:
    print(f"Fetching {label}...", flush=True)
    req = urllib.request.Request(
        url, headers={"User-Agent": "turnins-fyi/1.0 (FFXIV market helper)"}
    )
    with urllib.request.urlopen(req, timeout=60) as resp:
        return json.loads(resp.read())


def fetch_item_names(item_ids: list[int]) -> dict[int, str]:
    names = {}
    batch_size = 100
    for i in range(0, len(item_ids), batch_size):
        batch = item_ids[i : i + batch_size]
        ids_str = ",".join(str(x) for x in batch)
        url = f"https://xivapi.com/item?ids={ids_str}&columns=ID,Name"
        print(
            f"  Fetching item names {i+1}-{min(i+batch_size, len(item_ids))}"
            f" of {len(item_ids)}...",
            flush=True,
        )
        req = urllib.request.Request(
            url, headers={"User-Agent": "turnins-fyi/1.0 (FFXIV market helper)"}
        )
        try:
            with urllib.request.urlopen(req, timeout=30) as resp:
                data = json.loads(resp.read())
            for entry in data.get("Results", []):
                if entry.get("Name"):
                    names[entry["ID"]] = entry["Name"]
        except Exception as e:
            print(f"  Error fetching batch: {e}", file=sys.stderr)
        time.sleep(0.3)
    return names


def main():
    raw_recipes = fetch_json(TEAMCRAFT_RECIPES_URL, "TeamCraft recipes.json")

    # Index recipes two ways:
    #  - by (job, result item) for direct job-quest lookups
    #  - by result item alone, to find subcraft recipes regardless of job
    recipes_by_job_item: dict[tuple[int, int], dict] = {}
    recipes_by_item: dict[int, list[dict]] = {}
    for recipe in raw_recipes:
        job_id = recipe.get("job")
        if job_id not in JOBS:
            continue
        result_id = recipe["result"]
        recipes_by_job_item.setdefault((job_id, result_id), recipe)
        recipes_by_item.setdefault(result_id, []).append(recipe)

    # Walk the quest items breadth-first, expanding any ingredient that is
    # itself craftable into its own record. `queue` entries are
    # (job_id, item_id, turnin_level, turnin_count); turnin_level/count are
    # None for subcraft-only ingredients (they aren't quest hand-ins).
    queue: list[tuple[int, int, int | None, int | None]] = []
    for abbr, levels in QUEST_ITEMS.items():
        job_id = ABBR_TO_JOB_ID[abbr]
        for turnin_level, entries in levels:
            for item_id, count, _name in entries:
                queue.append((job_id, item_id, turnin_level, count))

    visited: set[tuple[int, int]] = set()
    records: list[dict] = []
    missing: list[tuple[str, int]] = []
    cross_class_picks: list[tuple[int, int, list[int]]] = []

    while queue:
        job_id, item_id, turnin_level, turnin_count = queue.pop(0)
        if (job_id, item_id) in visited:
            continue
        visited.add((job_id, item_id))

        recipe = recipes_by_job_item.get((job_id, item_id))
        if recipe is None:
            missing.append((JOBS[job_id][1], item_id))
            continue

        records.append(
            {
                "job": JOBS[job_id][1],
                "item_id": item_id,
                "recipe_id": recipe["id"],
                "recipe_level": recipe["lvl"],
                "turnin_level": turnin_level,
                "turnin_count": turnin_count,
                "yields": recipe.get("yields", 1),
                "ingredients": recipe["ingredients"],
            }
        )

        for ing in recipe["ingredients"]:
            ing_id = ing["id"]
            alts = recipes_by_item.get(ing_id)
            if not alts:
                continue  # base reagent (ore, shard, crystal, ...) — nothing to expand

            # Prefer a job for which this ingredient is already queued/visited
            # (e.g. it's also a direct quest hand-in for that job); otherwise
            # fall back to the lowest job ID with a recipe. The handful of
            # items craftable by multiple DoH jobs (basic ingots, rivets, ...)
            # use functionally-equivalent recipes that only differ in which
            # elemental shard/crystal they consume, so any valid pick is fine.
            candidate_jobs = sorted(r["job"] for r in alts)
            chosen_job = next(
                (j for j in candidate_jobs if (j, ing_id) in visited), None
            )
            if chosen_job is None:
                chosen_job = next(
                    (j for j in candidate_jobs if (j, ing_id) not in visited),
                    None,
                )
            if chosen_job is None:
                continue
            if len(candidate_jobs) > 1:
                cross_class_picks.append((chosen_job, ing_id, candidate_jobs))
            if (chosen_job, ing_id) not in visited:
                queue.append((chosen_job, ing_id, None, None))

    # Collect every item name we need (quest items + all ingredients, recursively)
    name_ids: set[int] = set()
    for record in records:
        name_ids.add(record["item_id"])
        for ing in record["ingredients"]:
            name_ids.add(ing["id"])

    print(f"Fetching names for {len(name_ids)} unique items...")
    item_names = fetch_item_names(sorted(name_ids))
    print(f"Got {len(item_names)} item names.")

    # Sort: job, then turnin_level (quest hand-ins first, by level; subcraft-only
    # entries — turnin_level None — follow, ordered by recipe level), then item_id.
    def sort_key(r):
        return (
            ABBR_TO_JOB_ID[r["job"]],
            r["turnin_level"] if r["turnin_level"] is not None else 999,
            r["recipe_level"],
            r["item_id"],
        )

    records.sort(key=sort_key)

    lines = []
    for record in records:
        out = {
            "job": record["job"],
            "item_id": record["item_id"],
            "item_name": item_names.get(record["item_id"], ""),
            "recipe_id": record["recipe_id"],
            "recipe_level": record["recipe_level"],
            "turnin_level": record["turnin_level"],
            "turnin_count": record["turnin_count"],
            "yields": record["yields"],
            "ingredients": [
                {
                    "item_id": ing["id"],
                    "item_name": item_names.get(ing["id"], ""),
                    "amount": ing["amount"],
                }
                for ing in record["ingredients"]
            ],
        }
        lines.append(json.dumps(out, ensure_ascii=False))

    with open("quest_recipes.jsonl", "w", encoding="utf-8") as f:
        f.write("\n".join(lines) + "\n")

    quest_count = sum(1 for r in records if r["turnin_level"] is not None)
    subcraft_count = len(records) - quest_count
    print(
        f"\nWritten quest_recipes.jsonl "
        f"({quest_count} quest hand-ins + {subcraft_count} subcraft recipes "
        f"= {len(records)} total)"
    )
    if missing:
        print(f"  {len(missing)} items had no matching recipe (unexpected): {missing}")
    if cross_class_picks:
        multi = {(iid, tuple(jobs)) for _, iid, jobs in cross_class_picks}
        print(f"  {len(multi)} ingredients are craftable by multiple jobs; picked one recipe each:")
        for iid, jobs in sorted(multi):
            picked = next(j for j, i, js in cross_class_picks if i == iid and tuple(js) == jobs)
            print(f"    item {iid}: candidates {[JOBS[j][1] for j in jobs]} -> used {JOBS[picked][1]}")


if __name__ == "__main__":
    main()
