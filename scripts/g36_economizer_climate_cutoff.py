"""Editable ASHRAE 90.1 fixed dry-bulb cutoff from pinned OBC climate groups.

Source: Buildings.Controls.OBC.ASHRAE.G36.Generic.AirEconomizerHighLimits,
Modelica Buildings commit d3d8e6f76514dfbc7d65574c6b5a777b686fe1b7.
The group code is a CDL parameter, never a simulator policy decision.
"""


CLIMATE_BLOCK_NAMES = (
    "economizer_climate_zone_group", "economizer_climate_code_real",
    "economizer_climate_low_boundary", "economizer_climate_mid_boundary",
    "economizer_climate_low_group", "economizer_climate_mid_group",
    "economizer_climate_cutoff_18", "economizer_climate_cutoff_21",
    "economizer_climate_cutoff_24", "economizer_climate_mid_select",
    "economizer_climate_cutoff_select", "economizer_use_climate_cutoff",
    "economizer_effective_fixed_cutoff",
)


def add_climate_fixed_dry_bulb_cutoff(graph, port, constant, binary, switch):
    # Code groups follow the three ASHRAE 90.1 fixed-cutoff branches in the
    # pinned OBC source. The default 5B group retains the previous 24 °C.
    group_note = ("Pinned OBC ASHRAE 90.1 fixed dry-bulb climate group: "
                  "1=1A/2A/3A/4A (18 °C); 2=5A/6A (21 °C); "
                  "3=1B/2B/3B/3C/4B/4C/5B/5C/6B/7/8 (24 °C). "
                  "Set this integer in the CDL graph; 3 is the bundled teaching choice.")
    for index in (1, 2):
        ahu = next(block for block in graph["blocks"] if block["name"] == f"ahu_{index}")
        ahu["blocks"].extend([
            {"name": "economizer_climate_zone_group", "block_type": "Buildings.Controls.OBC.CDL.Integers.Sources.Constant",
             "category": "elementary", "inputs": [], "outputs": [port("y", "Integer", "count")],
             "parameters": [{"name": "k", "type": "Integer", "value": 3}], "notes": group_note},
            {"name": "economizer_climate_code_real", "block_type": "Buildings.Controls.OBC.CDL.Conversions.IntegerToReal",
             "category": "elementary", "inputs": [port("u", "Integer", "count")],
             "outputs": [port("y", unit="count")], "notes": "Convert the editable climate group code for comparisons."},
            constant("economizer_climate_low_boundary", 1.5, "count", "Codes below 1.5 use the 18 °C fixed cutoff."),
            constant("economizer_climate_mid_boundary", 2.5, "count", "Codes below 2.5 and above 1.5 use the 21 °C fixed cutoff."),
            binary("economizer_climate_low_group", "Less", note="Select the OBC 18 °C climate group."),
            binary("economizer_climate_mid_group", "Less", note="Select the OBC 21 °C climate group or lower."),
            constant("economizer_climate_cutoff_18", 18.0, "°C", "Pinned OBC fixed cutoff for ASHRAE zones 1A, 2A, 3A, and 4A."),
            constant("economizer_climate_cutoff_21", 21.0, "°C", "Pinned OBC fixed cutoff for ASHRAE zones 5A and 6A."),
            constant("economizer_climate_cutoff_24", 24.0, "°C", "Pinned OBC fixed cutoff for the remaining ASHRAE zones."),
            switch("economizer_climate_mid_select", "°C", "Choose 21 °C for group 2, otherwise 24 °C."),
            switch("economizer_climate_cutoff_select", "°C", "Choose 18 °C for group 1, otherwise the 21/24 °C result."),
            {"name": "economizer_use_climate_cutoff", "block_type": "Buildings.Controls.OBC.CDL.Logical.Sources.Constant",
             "category": "elementary", "inputs": [], "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "k", "type": "Boolean", "value": True}],
             "notes": "Use the climate-group cutoff. Set false to use the manually edited cutoff constant."},
            switch("economizer_effective_fixed_cutoff", "°C", "Select climate mapping or the manual fixed dry-bulb cutoff."),
        ])
        old = [edge for edge in ahu["connections"] if edge["to"] == "economizer_fixed_dry_bulb_error.u2"]
        if len(old) != 1 or old[0]["from"] != "economizer_fixed_dry_bulb_cutoff.y":
            raise ValueError(f"Missing stock fixed dry-bulb cutoff in AHU {index}")
        ahu["connections"].remove(old[0])
        ahu["connections"].extend({"from": source, "to": target} for source, target in [
            ("economizer_climate_zone_group.y", "economizer_climate_code_real.u"),
            ("economizer_climate_code_real.y", "economizer_climate_low_group.u1"),
            ("economizer_climate_low_boundary.y", "economizer_climate_low_group.u2"),
            ("economizer_climate_code_real.y", "economizer_climate_mid_group.u1"),
            ("economizer_climate_mid_boundary.y", "economizer_climate_mid_group.u2"),
            ("economizer_climate_cutoff_21.y", "economizer_climate_mid_select.u1"),
            ("economizer_climate_mid_group.y", "economizer_climate_mid_select.u3"),
            ("economizer_climate_cutoff_24.y", "economizer_climate_mid_select.u2"),
            ("economizer_climate_cutoff_18.y", "economizer_climate_cutoff_select.u1"),
            ("economizer_climate_low_group.y", "economizer_climate_cutoff_select.u3"),
            ("economizer_climate_mid_select.y", "economizer_climate_cutoff_select.u2"),
            ("economizer_climate_cutoff_select.y", "economizer_effective_fixed_cutoff.u1"),
            ("economizer_use_climate_cutoff.y", "economizer_effective_fixed_cutoff.u3"),
            ("economizer_fixed_dry_bulb_cutoff.y", "economizer_effective_fixed_cutoff.u2"),
            ("economizer_effective_fixed_cutoff.y", "economizer_fixed_dry_bulb_error.u2"),
        ])
        ahu["notes"] += " Its fixed dry-bulb cutoff can follow an editable OBC climate group or the manual constant."
    graph["notes"] += " The fixed dry-bulb high limit has a graph-owned ASHRAE 90.1 climate-group selector."
