#!/usr/bin/env python3
"""
Generates data/base_materials.json and data/job_routes.json.

For each crafting job's quest items (levels 1-70), traces the full ingredient
tree to identify raw base materials, then looks up where to obtain each:
  - vendor in Ul'dah, Limsa Lominsa, or Gridania (preferred)
  - gathering node (BTN/MIN)
  - market board / cross-class subcraft

Sources:
  - data/quest_recipes.jsonl (ingredient trees)
  - TeamCraft shops.json, npcs.json, nodes.json, place-names.json (source lookup)

Run from the data/ directory: python3 gen_routes.py
"""
import json
import math
import sys
import urllib.request
from collections import defaultdict

# Home city for each job's guild
JOB_HOME_CITY = {
    "CRP": "Gridania",
    "BSM": "Limsa Lominsa",
    "ARM": "Limsa Lominsa",
    "GSM": "Ul'dah",
    "LTW": "Gridania",
    "WVR": "Ul'dah",
    "ALC": "Limsa Lominsa",
    "CUL": "Ul'dah",
}

# Substrings that identify the three main city-state zones
CITY_SUBSTRINGS = ["Limsa Lominsa", "Gridania", "Ul'dah"]

# TeamCraft gathering node types
NODE_TYPE = {0: "MIN", 1: "MIN", 2: "BTN", 3: "BTN"}

TEAMCRAFT_BASE = (
    "https://raw.githubusercontent.com/ffxiv-teamcraft/ffxiv-teamcraft"
    "/master/libs/data/src/lib/json"
)


def fetch_json(url: str, label: str) -> object:
    print(f"Fetching {label}...", flush=True)
    req = urllib.request.Request(
        url, headers={"User-Agent": "turnins-fyi/1.0 (FFXIV market helper)"}
    )
    with urllib.request.urlopen(req, timeout=120) as resp:
        return json.loads(resp.read())


def load_quest_recipes(path: str) -> list:
    entries = []
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if line:
                entries.append(json.loads(line))
    return entries


def compute_job_ingredients(
    job_abbr: str,
    quest_entries: list,
    job_recipe_map: dict,
    all_crafted_items: set,
    crafted_by_job: dict,
) -> dict:
    """
    BFS through a job's quest ingredient trees.

    Expands same-job subcrafts, stops at cross-class subcrafts (different job)
    and raw materials (not craftable at all).

    Returns {item_id: {name, amount, kind: 'raw'|'cross_class', made_by: str|None}}
    Amounts assume ceil(turnin_count/yields) crafts, no waste.
    """
    accumulated: dict = {}

    # Seed queue from all quest item ingredients
    queue: list = []
    for entry in quest_entries:
        crafts = math.ceil(entry["turnin_count"] / entry["yields"])
        for ing in entry["ingredients"]:
            queue.append((ing["item_id"], ing["item_name"], crafts * ing["amount"]))

    while queue:
        item_id, name, qty = queue.pop(0)

        if (job_abbr, item_id) in job_recipe_map:
            # Same-job subcraft: expand its ingredients
            sub = job_recipe_map[(job_abbr, item_id)]
            sub_crafts = math.ceil(qty / sub["yields"])
            for ing in sub["ingredients"]:
                queue.append((ing["item_id"], ing["item_name"], sub_crafts * ing["amount"]))
        elif item_id in all_crafted_items:
            # Craftable but by a different job: treat as cross-class (buy on MB or craft separately)
            made_by = crafted_by_job.get(item_id, [None])[0]
            acc = accumulated.setdefault(
                item_id,
                {"name": name, "amount": 0, "kind": "cross_class", "made_by": made_by},
            )
            acc["amount"] += qty
        else:
            # True raw material: shard, ore, log, animal part, food ingredient, etc.
            acc = accumulated.setdefault(
                item_id,
                {"name": name, "amount": 0, "kind": "raw", "made_by": None},
            )
            acc["amount"] += qty

    return accumulated


def build_source_maps(shops, npcs_raw, nodes_raw, place_names):
    """
    Build inverted lookup maps from TeamCraft data.

    Returns:
      shop_by_item: {item_id: [{npc_id, cost_gil}]}
      npc_info: {npc_id: {name, title, zone, city, x, y}}
      node_by_item: {item_id: [{zone, type, level, x, y}]}  (one entry per unique zone+type, lowest level)
    """
    # zone_id (int) → zone name string
    # places.json uses plain strings; place-names.json uses {en: ...} dicts
    zone_name_map: dict = {}
    for k, v in place_names.items():
        if isinstance(v, str):
            name = v
        elif isinstance(v, dict):
            name = v.get("en", "")
        else:
            name = ""
        try:
            zone_name_map[int(k)] = name
        except ValueError:
            pass

    def get_zone_name(zid):
        return zone_name_map.get(zid, f"Zone {zid}")

    def city_from_zone(zname):
        for city in CITY_SUBSTRINGS:
            if city in zname:
                return city
        return None

    # NPC info
    npc_info: dict = {}
    for npc_id_str, npc in npcs_raw.items():
        try:
            npc_id = int(npc_id_str)
        except ValueError:
            continue
        pos = npc.get("position") or {}
        zid = pos.get("zoneid", 0)
        zname = get_zone_name(zid)
        title = ""
        t = npc.get("title")
        if isinstance(t, dict):
            title = t.get("en", "")
        elif isinstance(t, str):
            title = t
        npc_info[npc_id] = {
            "name": npc.get("en") or f"NPC {npc_id}",
            "title": title,
            "zone": zname,
            "city": city_from_zone(zname),
            "x": pos.get("x", 0),
            "y": pos.get("y", 0),
        }

    # shop_by_item: scan all GilShop entries
    shop_by_item: dict = defaultdict(list)
    shop_list = shops if isinstance(shops, list) else list(shops.values())
    for shop in shop_list:
        if shop.get("type") != "GilShop":
            continue
        shop_npcs = shop.get("npcs") or []
        for trade in shop.get("trades") or []:
            cost_gil = 0
            for cur in trade.get("currencies") or []:
                if cur.get("id") == 1:
                    cost_gil = cur.get("amount", 0)
            for it in trade.get("items") or []:
                item_id = it.get("id")
                if item_id:
                    for npc_id in shop_npcs:
                        shop_by_item[item_id].append(
                            {"npc_id": npc_id, "cost_gil": cost_gil}
                        )

    # node_by_item: gather all node entries, then dedupe to lowest-level per (zone, type)
    raw_nodes: dict = defaultdict(list)
    for node_id_str, node in nodes_raw.items():
        zid = node.get("zoneid", 0)
        if not zid:
            continue  # skip nodes with no valid zone (zoneid=0)
        ntype = node.get("type", 0)
        type_str = NODE_TYPE.get(ntype, "GATHER")
        zname = get_zone_name(zid)
        level = node.get("level", 0)
        x = node.get("x", 0)
        y = node.get("y", 0)
        for item_id in node.get("items") or []:
            raw_nodes[item_id].append(
                {"zone": zname, "type": type_str, "level": level, "x": x, "y": y}
            )

    node_by_item: dict = {}
    for item_id, node_list in raw_nodes.items():
        best: dict = {}
        for n in node_list:
            key = (n["zone"], n["type"])
            if key not in best or n["level"] < best[key]["level"]:
                best[key] = n
        # Sort by level ascending so nodes[0] is the lowest-level (easiest) option
        node_by_item[item_id] = sorted(best.values(), key=lambda n: (n["level"], n["zone"]))

    return shop_by_item, npc_info, node_by_item


def get_vendor_sources(item_id: int, shop_by_item: dict, npc_info: dict) -> list:
    """Return deduplicated vendor source dicts, city vendors first, guild/material suppliers prioritized."""
    sources = []
    seen = set()
    for entry in shop_by_item.get(item_id) or []:
        npc_id = entry["npc_id"]
        info = npc_info.get(npc_id)
        if not info:
            continue
        key = (info["name"], info["zone"])
        if key in seen:
            continue
        seen.add(key)
        sources.append(
            {
                "npc_id": npc_id,
                "npc": info["name"],
                "title": info["title"],
                "zone": info["zone"],
                "city": info["city"],
                "coords": {"x": round(info["x"], 1), "y": round(info["y"], 1)},
                "cost_gil": entry["cost_gil"],
            }
        )
    # Preferred guild/material suppliers in city first, then other city vendors
    def sort_key(s):
        is_preferred = any(
            kw in s["title"] for kw in ("Guild Supplier", "Material Supplier")
        )
        return (0 if s["city"] else 1, 0 if is_preferred else 1, s["npc"])

    sources.sort(key=sort_key)
    return sources


def build_base_materials_db(
    all_raw_ids: set,
    all_entries: list,
    shop_by_item: dict,
    npc_info: dict,
    node_by_item: dict,
) -> dict:
    """Build the base_materials.json content."""
    names = {}
    for entry in all_entries:
        for ing in entry["ingredients"]:
            if ing["item_id"] in all_raw_ids:
                names[ing["item_id"]] = ing["item_name"]

    result = {}
    for item_id in sorted(all_raw_ids):
        vendor_sources = get_vendor_sources(item_id, shop_by_item, npc_info)
        gather_sources = node_by_item.get(item_id) or []
        result[str(item_id)] = {
            "name": names.get(item_id, ""),
            "vendor_sources": [
                {
                    "npc": s["npc"],
                    "title": s["title"],
                    "city": s["city"],
                    "zone": s["zone"],
                    "coords": s["coords"],
                    "cost_gil": s["cost_gil"],
                }
                for s in vendor_sources
            ],
            "gather_sources": [
                {
                    "zone": n["zone"],
                    "type": n["type"],
                    "level": n["level"],
                    "coords": {"x": round(n["x"], 1), "y": round(n["y"], 1)},
                }
                for n in gather_sources
            ],
        }
    return result


def build_job_route(
    job_abbr: str,
    ingredients: dict,
    shop_by_item: dict,
    npc_info: dict,
    node_by_item: dict,
) -> dict:
    """Build vendor_stops, gather_stops, market_board_items for one job."""
    home_city = JOB_HOME_CITY[job_abbr]

    raw_items = {iid: v for iid, v in ingredients.items() if v["kind"] == "raw"}
    cross_items = {iid: v for iid, v in ingredients.items() if v["kind"] == "cross_class"}

    # Route each raw item to a vendor stop, gathering zone, or unknown
    vendor_npc_items: dict = defaultdict(list)  # npc_id → [item entries]
    gather_zone_items: dict = defaultdict(list)  # (zone, type) → [item entries]
    unknown_items: list = []

    for item_id in sorted(raw_items):
        info = raw_items[item_id]
        vendor_srcs = get_vendor_sources(item_id, shop_by_item, npc_info)
        city_vendors = [v for v in vendor_srcs if v["city"]]

        if city_vendors:
            # Prefer home-city vendor; within that, prefer guild/material supplier
            home_vendors = [v for v in city_vendors if v["city"] == home_city]
            chosen = (home_vendors or city_vendors)[0]
            vendor_npc_items[chosen["npc_id"]].append(
                {
                    "item_id": item_id,
                    "name": info["name"],
                    "total_needed": info["amount"],
                    "cost_per_unit": chosen["cost_gil"],
                    "_vendor": chosen,
                }
            )
        else:
            nodes = node_by_item.get(item_id) or []
            if nodes:
                n = nodes[0]  # lowest-level / first zone
                gather_zone_items[(n["zone"], n["type"])].append(
                    {
                        "item_id": item_id,
                        "name": info["name"],
                        "total_needed": info["amount"],
                        "node_level": n["level"],
                        "coords": {"x": round(n["x"], 1), "y": round(n["y"], 1)},
                    }
                )
            else:
                unknown_items.append(
                    {
                        "item_id": item_id,
                        "name": info["name"],
                        "total_needed": info["amount"],
                        "note": "no vendor or gathering source found",
                    }
                )

    # Build vendor_stops: home city first, then alphabetical by city+npc
    vendor_stops = []
    for npc_id, items in vendor_npc_items.items():
        meta = items[0]["_vendor"]
        clean_items = [
            {k: v for k, v in it.items() if k != "_vendor"}
            for it in sorted(items, key=lambda x: x["name"])
        ]
        vendor_stops.append(
            {
                "city": meta["city"],
                "zone": meta["zone"],
                "npc": meta["npc"],
                "npc_title": meta["title"],
                "coords": meta["coords"],
                "items": clean_items,
            }
        )
    vendor_stops.sort(
        key=lambda s: (0 if s["city"] == home_city else 1, s["city"] or "", s["npc"])
    )

    # Build gather_stops
    gather_stops = []
    for (zone, gtype), items in sorted(gather_zone_items.items()):
        gather_stops.append(
            {
                "zone": zone,
                "type": gtype,
                "items": sorted(items, key=lambda x: (x["node_level"], x["name"])),
            }
        )

    # Market board: cross-class subcrafts + items with no source
    mb_items = []
    for item_id in sorted(cross_items):
        info = cross_items[item_id]
        made_by = info.get("made_by")
        note = (
            f"{made_by} subcraft — craft with {made_by} or buy on market board"
            if made_by
            else "cross-class subcraft — buy on market board"
        )
        mb_items.append(
            {
                "item_id": item_id,
                "name": info["name"],
                "total_needed": info["amount"],
                "note": note,
            }
        )
    mb_items.extend(sorted(unknown_items, key=lambda x: x["name"]))

    return {
        "vendor_stops": vendor_stops,
        "gather_stops": gather_stops,
        "market_board_items": mb_items,
    }


def main():
    print("Loading quest_recipes.jsonl...", flush=True)
    all_entries = load_quest_recipes("quest_recipes.jsonl")
    print(f"  {len(all_entries)} entries loaded")

    # Build recipe lookup structures
    job_recipe_map: dict = {}  # (job_abbr, item_id) → entry
    all_crafted_items: set = set()
    crafted_by_job: dict = defaultdict(list)  # item_id → [job_abbr]
    for entry in all_entries:
        job = entry["job"]
        iid = entry["item_id"]
        job_recipe_map[(job, iid)] = entry
        all_crafted_items.add(iid)
        if job not in crafted_by_job[iid]:
            crafted_by_job[iid].append(job)

    # Compute per-job base ingredients with total amounts
    print("\nComputing per-job base ingredients...", flush=True)
    per_job_ingredients: dict = {}
    all_raw_ids: set = set()
    for job_abbr in JOB_HOME_CITY:
        quest_entries = [
            e
            for e in all_entries
            if e["job"] == job_abbr and e["turnin_level"] is not None
        ]
        ingredients = compute_job_ingredients(
            job_abbr, quest_entries, job_recipe_map, all_crafted_items, crafted_by_job
        )
        per_job_ingredients[job_abbr] = ingredients
        raw_count = sum(1 for v in ingredients.values() if v["kind"] == "raw")
        cross_count = sum(1 for v in ingredients.values() if v["kind"] == "cross_class")
        print(f"  {job_abbr}: {raw_count} raw materials, {cross_count} cross-class subcrafts")
        for iid, info in ingredients.items():
            if info["kind"] == "raw":
                all_raw_ids.add(iid)
    print(f"Total unique raw base materials across all jobs: {len(all_raw_ids)}")

    # Fetch TeamCraft source data
    print("\nFetching TeamCraft data...", flush=True)
    shops = fetch_json(f"{TEAMCRAFT_BASE}/shops.json", "shops.json")
    shop_list = shops if isinstance(shops, list) else list(shops.values())
    print(f"  {len(shop_list)} shops loaded")

    npcs_raw = fetch_json(f"{TEAMCRAFT_BASE}/npcs.json", "npcs.json")
    print(f"  {len(npcs_raw)} NPCs loaded")

    nodes_raw = fetch_json(f"{TEAMCRAFT_BASE}/nodes.json", "nodes.json")
    print(f"  {len(nodes_raw)} nodes loaded")

    place_names = None
    for fname in ["places.json", "place-names.json", "zones.json"]:
        try:
            place_names = fetch_json(f"{TEAMCRAFT_BASE}/{fname}", fname)
            print(f"  {fname}: {len(place_names)} zone entries loaded")
            break
        except Exception as e:
            print(f"  {fname} unavailable: {e}", file=sys.stderr)
    if place_names is None:
        print("WARNING: no zone name file found; zone IDs will appear as 'Zone N'", file=sys.stderr)
        place_names = {}

    # Build lookup maps
    print("\nBuilding source lookup maps...", flush=True)
    shop_by_item, npc_info, node_by_item = build_source_maps(
        shop_list, npcs_raw, nodes_raw, place_names
    )
    city_vendor_items = sum(
        1
        for iid in all_raw_ids
        if any(s["city"] for s in get_vendor_sources(iid, shop_by_item, npc_info))
    )
    gather_only_items = sum(
        1
        for iid in all_raw_ids
        if not any(s["city"] for s in get_vendor_sources(iid, shop_by_item, npc_info))
        and iid in node_by_item
    )
    unknown_items_count = sum(
        1
        for iid in all_raw_ids
        if not any(s["city"] for s in get_vendor_sources(iid, shop_by_item, npc_info))
        and iid not in node_by_item
    )
    print(f"  City-vendor items: {city_vendor_items}")
    print(f"  Gather-only items: {gather_only_items}")
    print(f"  Unknown source:    {unknown_items_count}")

    # Write base_materials.json
    print("\nBuilding base_materials.json...", flush=True)
    base_materials = build_base_materials_db(
        all_raw_ids, all_entries, shop_by_item, npc_info, node_by_item
    )
    with open("base_materials.json", "w", encoding="utf-8") as f:
        json.dump(base_materials, f, ensure_ascii=False, indent=2)
    print(f"Written base_materials.json ({len(base_materials)} items)")

    # Write job_routes.json
    print("\nBuilding job_routes.json...", flush=True)
    job_routes: dict = {}
    for job_abbr in JOB_HOME_CITY:
        route = build_job_route(
            job_abbr,
            per_job_ingredients[job_abbr],
            shop_by_item,
            npc_info,
            node_by_item,
        )
        job_routes[job_abbr] = route
        vs = len(route["vendor_stops"])
        gs = len(route["gather_stops"])
        mb = len(route["market_board_items"])
        print(f"  {job_abbr}: {vs} vendor stops, {gs} gather zones, {mb} MB/unknown items")

    with open("job_routes.json", "w", encoding="utf-8") as f:
        json.dump(job_routes, f, ensure_ascii=False, indent=2)
    print("Written job_routes.json")

    print("\nNote: amounts assume NQ crafting with 100% success (theoretical minimum).")


if __name__ == "__main__":
    main()
