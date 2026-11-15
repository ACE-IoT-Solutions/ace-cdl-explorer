use ace_building_sim::{SimulationRow, MEASURE_COLUMNS};
use arrow_array::{ArrayRef, Float64Array, RecordBatch, UInt32Array};
use arrow_ipc::writer::StreamWriter;
use arrow_schema::{DataType, Field, Schema};
use std::sync::Arc;

pub(crate) fn arrow_batch(rows: &[SimulationRow], targets: &[String]) -> Result<Vec<u8>, String> {
    let mut fields = vec![Field::new("index", DataType::UInt32, false)];
    let mut arrays: Vec<ArrayRef> = vec![Arc::new(UInt32Array::from(
        rows.iter()
            .map(|row| u32::try_from(row.index).map_err(|_| "Invalid Arrow row index".to_string()))
            .collect::<Result<Vec<_>, _>>()?,
    ))];
    for name in MEASURE_COLUMNS
        .iter()
        .map(|name| name.to_string())
        .chain((1..=10).map(|zone| format!("zone_{zone:02}_c")))
        .chain((1..=10).map(|zone| format!("zone_{zone:02}_co2_ppm")))
        .chain((1..=10).map(|zone| format!("zone_{zone:02}_relative_humidity")))
        .chain((1..=10).map(|zone| format!("zone_{zone:02}_condensate_kg_s")))
        .chain((1..=10).map(|zone| format!("zone_{zone:02}_condensation_heat_kw")))
        .chain((1..=10).map(|zone| format!("zone_{zone:02}_occupied")))
        .chain((1..=10).map(|zone| format!("zone_{zone:02}_presence_raw")))
        .chain((1..=10).map(|zone| format!("zone_{zone:02}_occupant_count_actual")))
        .chain((1..=10).map(|zone| format!("zone_{zone:02}_occupant_count_sensor")))
        .chain((1..=10).map(|zone| format!("zone_{zone:02}_occupant_sensible_heat_kw")))
        .chain((1..=10).map(|zone| format!("zone_{zone:02}_airflow")))
        .chain((1..=10).map(|zone| format!("zone_{zone:02}_discharge_c")))
        .chain((1..=10).map(|zone| format!("zone_{zone:02}_actual_damper_position")))
        .chain((1..=10).map(|zone| format!("zone_{zone:02}_actual_reheat_valve_position")))
        .chain((1..=10).map(|zone| format!("zone_{zone:02}_measured_reheat_valve_position")))
        .chain(targets.iter().map(|target| format!("trace:{target}")))
    {
        fields.push(Field::new(&name, DataType::Float64, false));
        let values = rows
            .iter()
            .map(|row| {
                let value = if let Some(zone) = name
                    .strip_prefix("zone_")
                    .and_then(|s| s.strip_suffix("_actual_damper_position"))
                {
                    Some(
                        row.zone_actual_damper_position
                            [zone.parse::<usize>().expect("Known zone column") - 1],
                    )
                } else if let Some(zone) = name
                    .strip_prefix("zone_")
                    .and_then(|s| s.strip_suffix("_actual_reheat_valve_position"))
                {
                    Some(
                        row.zone_actual_reheat_valve_position
                            [zone.parse::<usize>().expect("Known zone column") - 1],
                    )
                } else if let Some(zone) = name
                    .strip_prefix("zone_")
                    .and_then(|s| s.strip_suffix("_measured_reheat_valve_position"))
                {
                    Some(
                        row.zone_measured_reheat_valve_position
                            [zone.parse::<usize>().expect("Known zone column") - 1],
                    )
                } else if let Some(zone) = name
                    .strip_prefix("zone_")
                    .and_then(|s| s.strip_suffix("_airflow"))
                {
                    Some(row.zone_airflow[zone.parse::<usize>().expect("Known zone column") - 1])
                } else if let Some(zone) = name
                    .strip_prefix("zone_")
                    .and_then(|s| s.strip_suffix("_discharge_c"))
                {
                    Some(
                        row.zone_discharge_c[zone.parse::<usize>().expect("Known zone column") - 1],
                    )
                } else if let Some(zone) = name
                    .strip_prefix("zone_")
                    .and_then(|s| s.strip_suffix("_co2_ppm"))
                {
                    Some(row.zone_co2_ppm[zone.parse::<usize>().expect("Known zone column") - 1])
                } else if let Some(zone) = name
                    .strip_prefix("zone_")
                    .and_then(|s| s.strip_suffix("_relative_humidity"))
                {
                    Some(
                        row.zone_relative_humidity
                            [zone.parse::<usize>().expect("Known zone column") - 1],
                    )
                } else if let Some(zone) = name
                    .strip_prefix("zone_")
                    .and_then(|s| s.strip_suffix("_condensate_kg_s"))
                {
                    Some(
                        row.zone_condensate_kg_s
                            [zone.parse::<usize>().expect("Known zone column") - 1],
                    )
                } else if let Some(zone) = name
                    .strip_prefix("zone_")
                    .and_then(|s| s.strip_suffix("_condensation_heat_kw"))
                {
                    Some(
                        row.zone_condensation_heat_kw
                            [zone.parse::<usize>().expect("Known zone column") - 1],
                    )
                } else if let Some(zone) = name
                    .strip_prefix("zone_")
                    .and_then(|s| s.strip_suffix("_c"))
                {
                    Some(row.zone_c[zone.parse::<usize>().expect("Known zone column") - 1])
                } else if let Some(zone) = name
                    .strip_prefix("zone_")
                    .and_then(|s| s.strip_suffix("_occupied"))
                {
                    Some(row.zone_occupied[zone.parse::<usize>().expect("Known zone column") - 1])
                } else if let Some(zone) = name
                    .strip_prefix("zone_")
                    .and_then(|s| s.strip_suffix("_presence_raw"))
                {
                    Some(
                        row.zone_presence_raw
                            [zone.parse::<usize>().expect("Known zone column") - 1],
                    )
                } else if let Some(zone) = name
                    .strip_prefix("zone_")
                    .and_then(|s| s.strip_suffix("_occupant_count_actual"))
                {
                    Some(
                        row.zone_occupant_count_actual
                            [zone.parse::<usize>().expect("Known zone column") - 1],
                    )
                } else if let Some(zone) = name
                    .strip_prefix("zone_")
                    .and_then(|s| s.strip_suffix("_occupant_count_sensor"))
                {
                    Some(
                        row.zone_occupant_count_sensor
                            [zone.parse::<usize>().expect("Known zone column") - 1],
                    )
                } else if let Some(zone) = name
                    .strip_prefix("zone_")
                    .and_then(|s| s.strip_suffix("_occupant_sensible_heat_kw"))
                {
                    Some(
                        row.zone_occupant_sensible_heat_kw
                            [zone.parse::<usize>().expect("Known zone column") - 1],
                    )
                } else if let Some(target) = name.strip_prefix("trace:") {
                    row.traces.get(target).copied()
                } else {
                    row.measure(&name)
                };
                value.ok_or_else(|| format!("Missing numeric Arrow column {name}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        arrays.push(Arc::new(Float64Array::from(values)));
    }
    let schema = Arc::new(Schema::new(fields));
    let batch = RecordBatch::try_new(schema.clone(), arrays).map_err(|e| e.to_string())?;
    let mut writer = StreamWriter::try_new(Vec::new(), &schema).map_err(|e| e.to_string())?;
    writer.write(&batch).map_err(|e| e.to_string())?;
    writer.into_inner().map_err(|e| e.to_string())
}
