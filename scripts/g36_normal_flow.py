"""Editable OBC DamperValves normal airflow-setpoint branch."""


def normal_flow_composite(port):
    flow = "normalized airflow"
    fraction = "fraction (0–1)"
    def source(name, value, unit):
        return {"name": name, "block_type": "Constant", "category": "elementary",
                "inputs": [], "outputs": [port("y", unit=unit)],
                "parameters": [{"name": "k", "value": value, "unit": unit}]}
    def unary(name, kind, input_type="Real", output_type="Boolean", parameters=None, unit=None):
        return {"name": name, "block_type": kind, "category": "elementary",
                "inputs": [port("u", input_type, unit)], "outputs": [port("y", output_type)],
                "parameters": parameters or []}
    def pair(name, kind, input_type="Real", output_type="Real", units=None, parameters=None, output_unit=None):
        return {"name": name, "block_type": kind, "category": "elementary",
                "inputs": [port("u1", input_type, units[0] if units else None),
                           port("u2", input_type, units[1] if units else None)],
                "outputs": [port("y", output_type, output_unit)], "parameters": parameters or []}
    def selector(name):
        return {"name": name, "block_type": "Switch", "category": "elementary",
                "inputs": [port("u1", unit=flow), port("u2", unit=flow), port("u3", "Boolean")],
                "outputs": [port("y", unit=flow)]}
    def line(name):
        return {"name": name, "block_type": "Buildings.Controls.OBC.CDL.Reals.Line", "category": "elementary",
                "inputs": [port("x1", unit=fraction), port("f1", unit=flow), port("u", unit=fraction),
                           port("x2", unit=fraction), port("f2", unit=flow)],
                "outputs": [port("y", unit=flow)],
                "parameters": [{"name": "limitBelow", "type": "Boolean", "value": True},
                               {"name": "limitAbove", "type": "Boolean", "value": True}]}
    def threshold(name, t, h, unit):
        return unary(name, "Buildings.Controls.OBC.CDL.Reals.GreaterThreshold", parameters=[
            {"name": "t", "value": t, "unit": unit}, {"name": "h", "value": h, "unit": unit}], unit=unit)
    def hold(name):
        return unary(name, "Buildings.Controls.OBC.CDL.Logical.TrueFalseHold", "Boolean", "Boolean", [
            {"name": "trueHoldDuration", "value": 600.0, "unit": "s"},
            {"name": "falseHoldDuration", "value": 0.0, "unit": "s"}])
    inputs = [port(name, unit=unit) for name, unit in [
        ("cooling_signal", fraction), ("heating_signal", fraction),
        ("cooling_min", flow), ("cooling_max", flow), ("minimum_flow", flow),
        ("heating_min", flow), ("heating_max", flow),
        ("supply_temp", "°C"), ("zone_temp", "°C"), ("discharge_temp", "°C"),
        ("heating_warm_margin", "K")]]
    blocks = [
        source("zero", 0.0, fraction), source("half", 0.5, fraction), source("one", 1.0, fraction),
        line("cooling_line"), line("heating_line"),
        threshold("cooling_detected", 0.01, 0.005, fraction), hold("cooling_hold"),
        threshold("heating_detected", 0.01, 0.008, fraction), hold("heating_hold"),
        pair("supply_minus_zone", "Subtract", units=("°C", "°C"), output_unit="K"),
        threshold("warm_supply", 0.25, 0.125, "K"),
        pair("cooling_and_warm", "And", "Boolean", "Boolean"),
        selector("cooling_supply_select"),
        pair("zone_plus_margin", "Add", units=("°C", "K"), output_unit="°C"),
        pair("warm_discharge", "Greater", output_type="Boolean", units=("°C", "°C"),
             parameters=[{"name":"h","value":0.25,"unit":"K"}]),
        selector("heating_discharge_select"),
        unary("not_cooling", "Not", "Boolean", "Boolean"),
        unary("not_heating", "Not", "Boolean", "Boolean"),
        pair("deadband", "And", "Boolean", "Boolean"),
        selector("deadband_select"), selector("normal_select"),
    ]
    edges = [
        ("zero.y", "cooling_line.x1"), ("cooling_min", "cooling_line.f1"),
        ("cooling_signal", "cooling_line.u"), ("one.y", "cooling_line.x2"),
        ("cooling_max", "cooling_line.f2"),
        ("half.y", "heating_line.x1"), ("heating_min", "heating_line.f1"),
        ("heating_signal", "heating_line.u"), ("one.y", "heating_line.x2"),
        ("heating_max", "heating_line.f2"),
        ("cooling_signal", "cooling_detected.u"), ("cooling_detected.y", "cooling_hold.u"),
        ("heating_signal", "heating_detected.u"), ("heating_detected.y", "heating_hold.u"),
        ("supply_temp", "supply_minus_zone.u1"), ("zone_temp", "supply_minus_zone.u2"),
        ("supply_minus_zone.y", "warm_supply.u"),
        ("cooling_hold.y", "cooling_and_warm.u1"), ("warm_supply.y", "cooling_and_warm.u2"),
        ("minimum_flow", "cooling_supply_select.u1"),
        ("cooling_line.y", "cooling_supply_select.u2"),
        ("cooling_and_warm.y", "cooling_supply_select.u3"),
        ("zone_temp", "zone_plus_margin.u1"),
        ("heating_warm_margin", "zone_plus_margin.u2"),
        ("discharge_temp", "warm_discharge.u1"), ("zone_plus_margin.y", "warm_discharge.u2"),
        ("heating_line.y", "heating_discharge_select.u1"),
        ("heating_min", "heating_discharge_select.u2"),
        ("warm_discharge.y", "heating_discharge_select.u3"),
        ("cooling_hold.y", "not_cooling.u"), ("heating_hold.y", "not_heating.u"),
        ("not_cooling.y", "deadband.u1"), ("not_heating.y", "deadband.u2"),
        ("minimum_flow", "deadband_select.u1"),
        ("heating_discharge_select.y", "deadband_select.u2"),
        ("deadband.y", "deadband_select.u3"),
        ("cooling_supply_select.y", "normal_select.u1"),
        ("deadband_select.y", "normal_select.u2"),
        ("cooling_hold.y", "normal_select.u3"),
        ("normal_select.y", "y"),
    ]
    return {"name": "normal_airflow_sequence", "block_type": "Composite", "category": "composite",
            "description": "OBC DamperValves normal cooling, deadband, and heating airflow setpoint mapping.",
            "notes": "Cooling and heating states hold for ten minutes. Warm AHU supply caps cooling flow at the active minimum; heating raises flow only after discharge is 3 K above zone temperature.",
            "inputs": inputs, "outputs": [port("y", unit=flow)], "blocks": blocks,
            "connections": [{"from": start, "to": end} for start, end in edges]}
