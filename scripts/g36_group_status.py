"""Route the pinned editable OBC GroupStatus into each five-zone AHU group."""

from obc_group_status import group_status


def add_obc_group_status_to_groups(graph):
    for index in (1, 2):
        group = next(block for block in graph["blocks"]
                     if block["name"] == f"zone_group_{index}")
        status = group_status(5)
        status["name"] = "obc_group_status"
        status["notes"] += " This instance includes all five zones and uses °C ports."
        for port in status["inputs"] + status["outputs"]:
            if port.get("unit") == "K":
                port["unit"] = "°C"
        group["blocks"].append(status)
        edges = group["connections"]
        for zone in range(1, 6):
            signals = {
                "zonOcc": f"override_{zone}",
                "u1Occ": "scheduled_occupied",
                "tNexOcc": "seconds_until_occupied",
                "uCooTim": f"zone_status_{zone}.yCooTim",
                "uWarTim": f"zone_status_{zone}.yWarTim",
                "u1OccHeaHig": f"zone_status_{zone}.yOccHeaHig",
                "u1HigOccCoo": f"zone_status_{zone}.yHigOccCoo",
                "u1UnoHeaHig": f"zone_status_{zone}.yUnoHeaHig",
                "THeaSetOff": f"unoccupied_heat_{zone}",
                "u1EndSetBac": f"zone_status_{zone}.yEndSetBac",
                "u1HigUnoCoo": f"zone_status_{zone}.yHigUnoCoo",
                "TCooSetOff": f"unoccupied_cool_{zone}",
                "u1EndSetUp": f"zone_status_{zone}.yEndSetUp",
                "TZon": f"room_{zone}",
                "u1Win": f"window_closed_{zone}",
            }
            edges.extend({"from": source,
                          "to": f"obc_group_status.{name}_{zone}"}
                         for name, source in signals.items())
        replacements = {
            "group_occupied.y": "obc_group_status.uGroOcc",
            "seconds_until_occupied": "obc_group_status.nexOcc",
            "optimal_cooldown_longest_5.y": "obc_group_status.yCooTim",
            "cooldown_status_any_5.y": "obc_group_status.yHigOccCoo",
            "optimal_warmup_longest_5.y": "obc_group_status.yWarTim",
            "warmup_status_any_5.y": "obc_group_status.yOccHeaHig",
            "mode_open_sum_5.y": "obc_group_status.yOpeWin",
            "mode_cold_sum_5.y": "obc_group_status.yColZon",
            "mode_hot_sum_5.y": "obc_group_status.yHotZon",
            "average_cold.y": "obc_group_status.ySetBac",
            "all_warm_5.y": "obc_group_status.yEndSetBac",
            "min_room_5.y": "obc_group_status.TZonMin",
            "average_hot.y": "obc_group_status.ySetUp",
            "all_cool_5.y": "obc_group_status.yEndSetUp",
        }
        for edge in edges:
            if edge["to"].startswith("obc_operation_mode.") or edge["from"] == "group_occupied.y":
                edge["from"] = replacements.get(edge["from"], edge["from"])
        group["notes"] += " Pinned OBC GroupStatus now supplies occupied status, group recovery extrema, cold/hot/open counts, setback/setup triggers, and zone temperature extrema to OperationMode."
