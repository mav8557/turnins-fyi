#!/usr/bin/env python3
"""
Generates data/turnin_recipes.jsonl — recipe data for all Grand Company supply
turn-in items across all DoH/DoL jobs. One JSON object per line, sorted by
job then turnin_level.

Gatherer jobs (MIN/BTN/FSH) have no crafting recipes and produce no output lines.

Sources:
  - data/items.json (turnin item IDs, levels, counts)
  - TeamCraft recipes.json (recipe ingredients by job)
  - XIVAPI (ingredient item names)

Run from the data/ directory: python3 gen_recipes.py
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
    16: ("Miner",         "MIN"),
    17: ("Botanist",      "BTN"),
    18: ("Fisher",        "FSH"),
}

TEAMCRAFT_RECIPES_URL = (
    "https://raw.githubusercontent.com/ffxiv-teamcraft/ffxiv-teamcraft"
    "/master/libs/data/src/lib/json/recipes.json"
)


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
            f"  Fetching ingredient names {i+1}-{min(i+batch_size, len(item_ids))}"
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
    with open("items.json") as f:
        items_data = json.load(f)
    item_names = items_data["item_names"]

    # Collect turnin entries for all jobs: [(job_id, turnin_level, item_id, count)]
    all_turnins: list[tuple[int, int, int, int]] = []
    for level_str, classes in items_data["levels"].items():
        for job_str, entries in classes.items():
            job_id = int(job_str)
            if job_id in JOBS:
                for entry in entries:
                    all_turnins.append((job_id, int(level_str), entry["item_id"], entry["count"]))
    all_turnins.sort()
    print(f"Found {len(all_turnins)} total turnin entries across {len(JOBS)} jobs")

    # Download TeamCraft recipes; index as {job_id: {result_item_id: recipe}}.
    # Keep first match per (job, result) — turnin items have exactly one recipe.
    raw_recipes = fetch_json(TEAMCRAFT_RECIPES_URL, "TeamCraft recipes.json")
    recipes_by_job: dict[int, dict[int, dict]] = {}
    for recipe in raw_recipes:
        job_id = recipe.get("job")
        if job_id in JOBS:
            job_map = recipes_by_job.setdefault(job_id, {})
            result_id = recipe["result"]
            if result_id not in job_map:
                job_map[result_id] = recipe
    for job_id, abbr_tuple in sorted(JOBS.items()):
        count = len(recipes_by_job.get(job_id, {}))
        print(f"  {abbr_tuple[1]}: {count} recipes indexed")

    # Collect all unique ingredient IDs across all relevant recipes
    ingredient_ids: set[int] = set()
    for job_id, turnin_level, item_id, _ in all_turnins:
        recipe = recipes_by_job.get(job_id, {}).get(item_id)
        if recipe:
            for ing in recipe["ingredients"]:
                ingredient_ids.add(ing["id"])

    print(f"Fetching names for {len(ingredient_ids)} unique ingredient items...")
    ingredient_names = fetch_item_names(sorted(ingredient_ids))
    print(f"Got {len(ingredient_names)} ingredient names.")

    # Build JSONL output
    lines = []
    missing_by_job: dict[str, list] = {}
    for job_id, turnin_level, item_id, turnin_count in all_turnins:
        abbr = JOBS[job_id][1]
        recipe = recipes_by_job.get(job_id, {}).get(item_id)
        if recipe is None:
            missing_by_job.setdefault(abbr, []).append((turnin_level, item_id))
            continue

        record = {
            "job": abbr,
            "item_id": item_id,
            "item_name": item_names.get(str(item_id), ""),
            "recipe_id": recipe["id"],
            "recipe_level": recipe["lvl"],
            "turnin_level": turnin_level,
            "turnin_count": turnin_count,
            "yields": recipe.get("yields", 1),
            "ingredients": [
                {
                    "item_id": ing["id"],
                    "item_name": ingredient_names.get(ing["id"], ""),
                    "amount": ing["amount"],
                }
                for ing in recipe["ingredients"]
            ],
        }
        lines.append(json.dumps(record, ensure_ascii=False))

    with open("turnin_recipes.jsonl", "w", encoding="utf-8") as f:
        f.write("\n".join(lines) + "\n")

    print(f"\nWritten turnin_recipes.jsonl ({len(lines)} recipes)")
    if missing_by_job:
        for abbr, items in sorted(missing_by_job.items()):
            print(f"  {abbr}: {len(items)} items with no recipe (expected for gatherers)")


if __name__ == "__main__":
    main()
