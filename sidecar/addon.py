"""mitmproxy addon for TaskBarHero Dashboard.

Captures responses from thebackend.io and outputs JSON events to stdout.
The Rust/Tauri backend reads these events and manages persistent state.
"""

import json
import re
import sys
from datetime import datetime, timezone

from mitmproxy import http


# ---------------------------------------------------------------------------
# Utility helpers (minimal duplicates from the old Python project)
# ---------------------------------------------------------------------------

def utc_now():
    return datetime.now(timezone.utc)


def safe_int(x, default=None):
    try:
        return int(x)
    except Exception:
        return default


def parse_jsonish_list(value):
    if value is None:
        return []
    if isinstance(value, list):
        return [str(x) for x in value]
    if isinstance(value, (int, float)):
        return [str(value)]
    if isinstance(value, str):
        value = value.strip()
        if not value:
            return []
        try:
            parsed = json.loads(value)
            if isinstance(parsed, list):
                return [str(x) for x in parsed]
            return [str(parsed)]
        except Exception:
            return [value]
    return []


BOX_NAMES = {
    910651: "Common Treasure Chest",
    920651: "Stage Treasure Chest",
}


def box_label(box_id):
    bid = safe_int(box_id)
    if bid in BOX_NAMES:
        return BOX_NAMES[bid]
    if bid is None:
        return "Unknown Chest"
    s = str(bid)
    if s.startswith("910"):
        return f"Common Treasure Chest ({bid})"
    if s.startswith("920"):
        return f"Stage Treasure Chest ({bid})"
    return f"Box {bid}"


# ---------------------------------------------------------------------------
# JSON extraction from response body
# ---------------------------------------------------------------------------

def extract_chests_from_any_json(obj):
    """Recursively find chest/box data in nested JSON."""
    found = []
    if isinstance(obj, dict):
        if "result" in obj and isinstance(obj["result"], str):
            try:
                found.extend(extract_chests_from_any_json(json.loads(obj["result"])))
            except Exception:
                pass
        data = obj.get("data")
        if isinstance(data, dict) and isinstance(data.get("boxes"), list):
            found.extend([b for b in data["boxes"] if isinstance(b, dict)])
        for k, v in obj.items():
            if k == "items":
                if isinstance(v, str):
                    try:
                        v = json.loads(v)
                    except Exception:
                        pass
                if isinstance(v, list):
                    for it in v:
                        if isinstance(it, dict) and ("claimableAt" in it or "rewardItemId" in it):
                            found.append(it)
            else:
                found.extend(extract_chests_from_any_json(v))
    elif isinstance(obj, list):
        for x in obj:
            found.extend(extract_chests_from_any_json(x))
    return found


def extract_added_from_any_json(obj):
    """Extract immediate rewards (boss drops, direct inventory additions)."""
    found = []
    if isinstance(obj, dict):
        if "result" in obj and isinstance(obj["result"], str):
            try:
                found.extend(extract_added_from_any_json(json.loads(obj["result"])))
            except Exception:
                pass
        data = obj.get("data")
        if isinstance(data, dict) and isinstance(data.get("added"), list):
            for it in data["added"]:
                if isinstance(it, dict):
                    found.append(it)
        for k in ("added", "rewards", "reward", "items"):
            v = obj.get(k)
            if isinstance(v, str):
                try:
                    v = json.loads(v)
                except Exception:
                    pass
            if isinstance(v, list):
                for it in v:
                    if isinstance(it, dict) and ("itemId" in it or "item_id" in it):
                        if "claimableAt" not in it and "rewardItemId" not in it:
                            found.append(it)
        for v in obj.values():
            if isinstance(v, (dict, list)):
                found.extend(extract_added_from_any_json(v))
    elif isinstance(obj, list):
        for x in obj:
            found.extend(extract_added_from_any_json(x))

    # Deduplicate by key
    dedup = {}
    for it in found:
        key = str(it.get("itemKey") or it.get("inDate") or it.get("uuid") or it.get("itemId") or id(it))
        dedup[key] = it
    return list(dedup.values())


# ---------------------------------------------------------------------------
# processBox request parsing
# ---------------------------------------------------------------------------

def get_processbox_info(flow):
    """Parse a processBox request from the game client."""
    try:
        req = json.loads(flow.request.get_text(strict=False) or "{}")
    except Exception:
        return None
    if req.get("functionName") != "inventory":
        return None
    body = req.get("functionBody", {}).get("body", {})
    if not isinstance(body, dict) or body.get("action") != "processBox":
        return None

    created = []
    raw = body.get("createItemList")
    if isinstance(raw, str):
        try:
            raw = json.loads(raw)
        except Exception:
            raw = []
    if isinstance(raw, list):
        for x in raw:
            if isinstance(x, dict):
                item_id = safe_int(x.get("itemId"))
                count = safe_int(x.get("count"), 0)
                drop_key = safe_int(x.get("dropKey"))
                created.append({
                    "itemId": item_id,
                    "count": count,
                    "dropKey": drop_key,
                    "name": box_label(item_id),
                })

    return {
        "tn": body.get("tn"),
        "isReset": str(body.get("isReset", "")).lower() == "true",
        "created": created,
        "at": utc_now().isoformat(),
    }


def describe_processbox_request(flow):
    info = get_processbox_info(flow)
    if not info or not info.get("created"):
        return None
    parts = [f"{x['name']} x{x['count']} dropKey={x['dropKey']}" for x in info["created"]]
    return f"processBox requested tn={info.get('tn')} reset={info.get('isReset')}: " + ", ".join(parts)


def mark_claimed_from_backend_request(flow):
    """Extract consumed item keys from request to mark as claimed."""
    try:
        req = json.loads(flow.request.get_text(strict=False) or "{}")
    except Exception:
        return []
    if req.get("functionName") != "inventory":
        return []
    body = req.get("functionBody", {}).get("body", {})
    if not isinstance(body, dict):
        return []
    action = str(body.get("action", ""))
    keys = []
    if action == "processBox":
        for field in ("useItemKeyList", "useItemKeys", "openItemKeyList", "boxKeyList"):
            keys.extend(parse_jsonish_list(body.get(field)))
    elif action == "exchange":
        for field in ("itemKey", "itemKeys", "useItemKeyList"):
            keys.extend(parse_jsonish_list(body.get(field)))
    else:
        for field in ("useItemKeyList", "itemKey", "itemKeys", "openItemKeyList"):
            keys.extend(parse_jsonish_list(body.get(field)))
    return [k for k in keys if k and not k.startswith("manual-")]


# ---------------------------------------------------------------------------
# Output helpers — print JSON lines to stdout
# ---------------------------------------------------------------------------

def emit(event_type, **kwargs):
    """Print a JSON event to stdout for the Rust backend to consume."""
    event = {"type": event_type, **kwargs}
    print(json.dumps(event, ensure_ascii=False, default=str), flush=True)


# ---------------------------------------------------------------------------
# mitmproxy addon entry point
# ---------------------------------------------------------------------------

def response(flow: http.HTTPFlow):
    """Called for every HTTP response passing through the proxy."""
    try:
        _handle_response(flow)
    except Exception as e:
        print(f"[TBH-sidecar] addon error: {e}", file=sys.stderr)


def _handle_response(flow: http.HTTPFlow):
    host = flow.request.pretty_host
    path = flow.request.path
    if "thebackend.io" not in host:
        return
    if not ("/backend-function/base/v1" in path or "UserInventory" in path or "SteamItemInfo" in path):
        return

    text = flow.response.get_text(strict=False) if flow.response else ""
    if not text:
        return

    try:
        obj = json.loads(text)
    except Exception:
        return

    source = f"{flow.request.method} {host}{path.split('?')[0]}"

    # 1. Mark claimed from request body
    claimed_keys = []
    if "/backend-function/base/v1" in path:
        claimed_keys = mark_claimed_from_backend_request(flow)
        if claimed_keys:
            emit("claimed", count=len(claimed_keys), keys=claimed_keys)

    # 2. Detect processBox events
    pb_info = None
    pb_desc = None
    if "/backend-function/base/v1" in path:
        pb_info = get_processbox_info(flow)
        pb_desc = describe_processbox_request(flow) if pb_info else None

    if pb_desc and pb_info:
        emit("process_box", info=pb_info, description=pb_desc)

    # 3. Extract immediate added items
    added_items = extract_added_from_any_json(obj)
    if added_items:
        emit("added_items", count=len(added_items), source=source, items=added_items)

    # 4. Extract chests
    chests = extract_chests_from_any_json(obj)
    if chests:
        if len(chests) >= 40 or "UserInventory" in path:
            emit("chests_synced", count=len(chests), old=0, source=source, chests=chests)
        else:
            emit("chests_upserted", added=len(chests), updated=0, source=source, chests=chests)

    # 5. Emit claimed keys if any
    if claimed_keys:
        pass
