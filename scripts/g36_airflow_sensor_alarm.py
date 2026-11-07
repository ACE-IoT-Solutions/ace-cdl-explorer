"""Diagnose a VAV flow sensor reading after its AHU fan is proven off."""


def add_airflow_sensor_alarms(graph, port, constant, binary):
    def wire(source, target):
        return {"from": source, "to": target}

    for index in range(1, 11):
        name = f"zone_{index:02}"
        zone = next(block for block in graph["blocks"] if block["name"] == name)
        zone["blocks"].extend([
            {"name": "sensor_fan_off", "block_type": "Not", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
             "notes": "Diagnose the flow sensor only while the AHU fan motor is proven off."},
            {"name": "sensor_fan_off_delay", "block_type": "TrueDelay", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "delayTime", "value": 600.0, "unit": "s"},
                            {"name": "delayOnInit", "type": "Boolean", "value": True}],
             "notes": "Require ten continuous minutes of fan-off proof before checking residual flow."},
            constant("sensor_cooling_design_flow", 0.95, "normalized airflow",
                     "Editable design cooling maximum: 0.2 minimum plus 0.75 cooling span in this example."),
            {"name": "sensor_ten_percent", "block_type": "MultiplyByParameter", "category": "elementary",
             "inputs": [port("u", unit="normalized airflow")],
             "outputs": [port("y", unit="normalized airflow")],
             "parameters": [{"name": "k", "type": "Real", "value": 0.1}],
             "notes": "A reading above ten percent of design cooling flow indicates sensor calibration trouble while the fan is off."},
            {"name": "sensor_residual_flow", "block_type": "Greater", "category": "elementary",
             "inputs": [port("u1", unit="normalized airflow"),
                        port("u2", unit="normalized airflow")],
             "outputs": [port("y", "Boolean")],
             "notes": "Measured primary flow exceeds ten percent of design cooling flow."},
            binary("sensor_alarm_condition", "And", "Boolean", "Boolean",
                   "Fan proven off for ten minutes and measured flow remains above the design threshold."),
            {"name": "sensor_alarm_zero", "block_type": "Buildings.Controls.OBC.CDL.Integers.Sources.Constant",
             "category": "elementary", "inputs": [], "outputs": [port("y", "Integer", "level")],
             "parameters": [{"name": "k", "type": "Integer", "value": 0}],
             "notes": "No airflow sensor calibration alarm."},
            {"name": "sensor_alarm_three", "block_type": "Buildings.Controls.OBC.CDL.Integers.Sources.Constant",
             "category": "elementary", "inputs": [], "outputs": [port("y", "Integer", "level")],
             "parameters": [{"name": "k", "type": "Integer", "value": 3}],
             "notes": "Level 3 airflow sensor calibration alarm."},
            {"name": "sensor_alarm_select", "block_type": "Buildings.Controls.OBC.CDL.Integers.Switch",
             "category": "elementary",
             "inputs": [port("u1", "Integer", "level"), port("u2", "Boolean"),
                        port("u3", "Integer", "level")],
             "outputs": [port("y", "Integer", "level")],
             "notes": "Publish Level 3 only when the fan-off residual-flow condition is true."},
        ])
        zone["connections"].extend(wire(source, target) for source, target in [
            ("supply_fan_proven", "sensor_fan_off.u"),
            ("sensor_fan_off.y", "sensor_fan_off_delay.u"),
            ("sensor_cooling_design_flow.y", "sensor_ten_percent.u"),
            ("measured_airflow", "sensor_residual_flow.u1"),
            ("sensor_ten_percent.y", "sensor_residual_flow.u2"),
            ("sensor_fan_off_delay.y", "sensor_alarm_condition.u1"),
            ("sensor_residual_flow.y", "sensor_alarm_condition.u2"),
            ("sensor_alarm_three.y", "sensor_alarm_select.u1"),
            ("sensor_alarm_condition.y", "sensor_alarm_select.u2"),
            ("sensor_alarm_zero.y", "sensor_alarm_select.u3"),
            ("sensor_alarm_select.y", "airflow_sensor_alarm_level"),
        ])
        zone["outputs"].append(port("airflow_sensor_alarm_level", "Integer", "level",
                                    "Airflow Sensor Calibration Alarm", "Alarm"))
        point = f"{name}_airflow_sensor_alarm_level"
        graph["outputs"].append(port(point, "Integer", "level",
                                     f"Zone {index} Airflow Sensor Calibration Alarm", "Alarm"))
        graph["connections"].append(wire(f"{name}.airflow_sensor_alarm_level", point))
        zone["notes"] += " A Level 3 airflow-sensor calibration alarm reports residual measured flow above ten percent of design after ten minutes of AHU fan-off proof."
    graph["notes"] += " Fan-off airflow sensor calibration alarms are editable CDL and use physical motor status and flow feedback."
