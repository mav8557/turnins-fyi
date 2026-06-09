#!/usr/bin/env python3
"""
One-time script to produce data/items.json from TeamCraft's gc-supply raw data.
Structure of gc_supply_raw.json:
  { level_key_1-100: { class_job_id_8-18: [{ itemId, count, reward }] } }
where level_key maps to player level 1-100 and class_job_id matches FFXIV ClassJob IDs.
"""
import json
import time
import sys
import urllib.request
import urllib.error

DOH_DOL = {
    8: ("Carpenter", "CRP"),
    9: ("Blacksmith", "BSM"),
    10: ("Armorer", "ARM"),
    11: ("Goldsmith", "GSM"),
    12: ("Leatherworker", "LTW"),
    13: ("Weaver", "WVR"),
    14: ("Alchemist", "ALC"),
    15: ("Culinarian", "CUL"),
    16: ("Miner", "MIN"),
    17: ("Botanist", "BTN"),
    18: ("Fisher", "FSH"),
}
DOH_DOL_STRS = set(str(i) for i in DOH_DOL)


def fetch_item_names(item_ids: list[int]) -> dict[int, str]:
    names = {}
    batch_size = 100
    for i in range(0, len(item_ids), batch_size):
        batch = item_ids[i : i + batch_size]
        ids_str = ",".join(str(x) for x in batch)
        url = f"https://xivapi.com/item?ids={ids_str}&columns=ID,Name"
        print(f"  Fetching names {i+1}-{min(i+batch_size, len(item_ids))} of {len(item_ids)}...", flush=True)
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
    with open("gc_supply_raw.json") as f:
        raw = json.load(f)

    # Collect all unique item IDs for DoH/DoL across all levels
    all_ids: set[int] = set()
    for level_str, classes in raw.items():
        for cj_str, items in classes.items():
            if cj_str in DOH_DOL_STRS:
                for item in items:
                    all_ids.add(item["itemId"])

    print(f"Fetching names for {len(all_ids)} unique items...")
    item_names = fetch_item_names(sorted(all_ids))
    print(f"Got {len(item_names)} names.")

    # Build output: { levels: { "1"-"100": { "8"-"18": [{ item_id, count }] } }, item_names: { id: name } }
    levels: dict[str, dict] = {}
    for level_str in sorted(raw.keys(), key=int):
        classes = raw[level_str]
        class_entries: dict[str, list] = {}
        for cj_str, items in classes.items():
            if cj_str not in DOH_DOL_STRS or not items:
                continue
            class_entries[cj_str] = [
                {"item_id": item["itemId"], "count": item["count"]}
                for item in items
            ]
        if class_entries:
            levels[level_str] = class_entries

    output = {
        "item_names": {str(k): v for k, v in item_names.items()},
        "jobs": [
            {"class_job_id": cj_id, "name": name, "abbr": abbr}
            for cj_id, (name, abbr) in sorted(DOH_DOL.items())
        ],
        "levels": levels,
    }

    with open("items.json", "w") as f:
        json.dump(output, f)
    print(f"Written items.json ({len(levels)} level entries, {len(item_names)} item names)")


if __name__ == "__main__":
    main()
