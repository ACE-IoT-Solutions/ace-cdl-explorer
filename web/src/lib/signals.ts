import { edgeEndpoints, type Block, type Connector, type Row } from './types'

export interface GraphSignal { id: string; label: string; kind: 'input' | 'output' | 'connection' | 'physical'; type: string; unit: string; brickClass?: string }

const physicalFields: Array<[keyof Row, string, string, string?]> = [
  ['schedule_occupied', 'Building Occupied Schedule', 'Boolean (0/1)', 'Point'],
  ['outdoor_c', 'Outside Air Temperature Sensor', '°C', 'Outside_Air_Temperature_Sensor'],
  ['avg_zone_c', 'Average Zone Air Temperature · calculated', '°C'],
  ['requested_airflow', 'VAV supply air flow request · total', 'normalized airflow'],
  ['delivered_airflow', 'Supply air flow delivered · total', 'normalized airflow'],
  ['ahu_1_supply_setpoint_c', 'AHU 1 Supply Air Temperature Setpoint', '°C', 'Supply_Air_Temperature_Setpoint'],
  ['ahu_1_supply_actual_c', 'AHU 1 Supply Air Temperature Sensor', '°C', 'Supply_Air_Temperature_Sensor'],
  ['ahu_2_supply_setpoint_c', 'AHU 2 Supply Air Temperature Setpoint', '°C', 'Supply_Air_Temperature_Setpoint'],
  ['ahu_2_supply_actual_c', 'AHU 2 Supply Air Temperature Sensor', '°C', 'Supply_Air_Temperature_Sensor'],
  ['ahu_1_duct_pressure', 'AHU 1 Duct Static Pressure · normalized', 'normalized pressure'],
  ['ahu_2_duct_pressure', 'AHU 2 Duct Static Pressure · normalized', 'normalized pressure'],
  ['ahu_1_outdoor_air_fraction', 'AHU 1 Outside Air Fraction · calculated', 'fraction (0–1)'],
  ['ahu_2_outdoor_air_fraction', 'AHU 2 Outside Air Fraction · calculated', 'fraction (0–1)'],
  ['ahu_1_intake_opening', 'AHU 1 Combined Intake Opening · calculated', 'fraction (0–1)'],
  ['ahu_2_intake_opening', 'AHU 2 Combined Intake Opening · calculated', 'fraction (0–1)'],
  ['ahu_1_actual_fan_speed', 'AHU 1 Physical Supply Fan Speed', 'fraction (0–1)'],
  ['ahu_2_actual_fan_speed', 'AHU 2 Physical Supply Fan Speed', 'fraction (0–1)'],
  ['ahu_1_measured_fan_speed', 'AHU 1 Supply Fan Speed Sensor', 'fraction (0–1)', 'Motor_Speed_Sensor'],
  ['ahu_2_measured_fan_speed', 'AHU 2 Supply Fan Speed Sensor', 'fraction (0–1)', 'Motor_Speed_Sensor'],
  ['ahu_1_actual_outdoor_damper_position', 'AHU 1 Outdoor Air Damper Position Sensor', 'fraction (0–1)', 'Damper_Position_Sensor'],
  ['ahu_2_actual_outdoor_damper_position', 'AHU 2 Outdoor Air Damper Position Sensor', 'fraction (0–1)', 'Damper_Position_Sensor'],
  ['ahu_1_actual_return_damper_position', 'AHU 1 Return Air Damper Position Sensor', 'fraction (0–1)', 'Damper_Position_Sensor'],
  ['ahu_2_actual_return_damper_position', 'AHU 2 Return Air Damper Position Sensor', 'fraction (0–1)', 'Damper_Position_Sensor'],
  ['ahu_1_heating_coil_valve_position', 'AHU 1 Heating Coil Valve Command', 'fraction (0–1)', 'Heating_Valve_Command'],
  ['ahu_2_heating_coil_valve_position', 'AHU 2 Heating Coil Valve Command', 'fraction (0–1)', 'Heating_Valve_Command'],
  ['ahu_1_cooling_coil_valve_position', 'AHU 1 Cooling Coil Valve Command', 'fraction (0–1)', 'Cooling_Valve_Command'],
  ['ahu_2_cooling_coil_valve_position', 'AHU 2 Cooling Coil Valve Command', 'fraction (0–1)', 'Cooling_Valve_Command'],
  ['ahu_1_heating_coil_request_kw', 'AHU 1 Heating Coil Request', 'kW'],
  ['ahu_2_heating_coil_request_kw', 'AHU 2 Heating Coil Request', 'kW'],
  ['ahu_1_cooling_coil_request_kw', 'AHU 1 Cooling Coil Request', 'kW'],
  ['ahu_2_cooling_coil_request_kw', 'AHU 2 Cooling Coil Request', 'kW'],
  ['ahu_1_heating_coil_delivered_kw', 'AHU 1 Heating Coil Delivered', 'kW'],
  ['ahu_2_heating_coil_delivered_kw', 'AHU 2 Heating Coil Delivered', 'kW'],
  ['ahu_1_cooling_coil_delivered_kw', 'AHU 1 Cooling Coil Delivered', 'kW'],
  ['ahu_2_cooling_coil_delivered_kw', 'AHU 2 Cooling Coil Delivered', 'kW'],
  ['ahu_1_outdoor_airflow', 'AHU 1 Outside Air Flow Sensor', 'normalized airflow', 'Outside_Air_Flow_Sensor'],
  ['ahu_2_outdoor_airflow', 'AHU 2 Outside Air Flow Sensor', 'normalized airflow', 'Outside_Air_Flow_Sensor'],
  ['ahu_1_mixed_air_c', 'AHU 1 Mixed Air Temperature · calculated', '°C'],
  ['ahu_2_mixed_air_c', 'AHU 2 Mixed Air Temperature · calculated', '°C'],
  ['cool_request_kw', 'Cooling coil request', 'kW'],
  ['cool_kw', 'Cooling delivered', 'kW'],
  ['heat_request_kw', 'Heating coil and reheat request', 'kW'],
  ['heat_kw', 'Heating delivered', 'kW'],
  ['heat_pump_heating_used_kw', 'Heat pump heating used', 'kW'],
  ['heat_pump_cooling_used_kw', 'Heat pump cooling used', 'kW'],
  ['heat_pump_electric_kw', 'Heat Pump Compressor Electricity · calculated', 'kW', 'Point'],
  ['heat_pump_ambient_exchange_kw', 'Heat Pump Ambient Exchange (+ absorbed, − rejected) · calculated', 'kW', 'Point'],
  ['heat_pump_available_capacity_kw', 'Heat Pump Thermal Capacity at Conditions · before status', 'kW', 'Point'],
  ['heat_pump_heating_available_capacity_kw', 'Heat Pump Heating Capacity at Water Temperature · physical', 'kW', 'Point'],
  ['heat_pump_cooling_available_capacity_kw', 'Heat Pump Cooling Capacity at Water Temperature · physical', 'kW', 'Point'],
  ['heat_pump_part_load_fraction', 'Heat Pump Delivered Part Load · physical', 'fraction (0–1)', 'Point'],
  ['heat_pump_cop_multiplier', 'Heat Pump Part-Load COP Multiplier · physical', 'unitless', 'Point'],
  ['heat_pump_hot_water_cop_multiplier', 'Heat Pump Hot-Water COP Multiplier · physical', 'unitless', 'Point'],
  ['heat_pump_chilled_water_cop_multiplier', 'Heat Pump Chilled-Water COP Multiplier · physical', 'unitless', 'Point'],
  ['heat_pump_heating_cop', 'Heat Pump Heating COP · equipment assumption', 'unitless', 'Point'],
  ['heat_pump_cooling_cop', 'Heat Pump Cooling COP · equipment assumption', 'unitless', 'Point'],
  ['heat_pump_available', 'Heat Pump Available Status · raw', 'Boolean (0/1)', 'Status'],
  ['boiler_available', 'Boiler Available Status · raw', 'Boolean (0/1)', 'Status'],
  ['chiller_available', 'Chiller Available Status · raw', 'Boolean (0/1)', 'Status'],
  ['heat_pump_running', 'Heat Pump Running Status · raw', 'Boolean (0/1)', 'Status'],
  ['hot_water_plant_proven', 'Hot Water Plant Heating Proven · physical', 'Boolean (0/1)', 'Status'],
  ['plant_heat_pump_start_fault', 'Plant Heat Pump Start Fault · CDL', 'Boolean (0/1)', 'Alarm'],
  ['plant_boiler_start_fault', 'Plant Boiler Start Fault · CDL', 'Boolean (0/1)', 'Alarm'],
  ['plant_chiller_start_fault', 'Plant Chiller Start Fault · CDL', 'Boolean (0/1)', 'Alarm'],
  ['boiler_running', 'Boiler Running Status · raw', 'Boolean (0/1)', 'Status'],
  ['chiller_running', 'Chiller Running Status · raw', 'Boolean (0/1)', 'Status'],
  ['boiler_used_kw', 'Boiler heat used', 'kW'],
  ['chiller_used_kw', 'Chiller cooling used', 'kW'],
  ['hot_water_supply_temp_c', 'Hot Water Supply Temperature Sensor', '°C', 'Hot_Water_Supply_Temperature_Sensor'],
  ['hot_water_supply_setpoint_c', 'Hot Water Supply Temperature Setpoint', '°C', 'Supply_Hot_Water_Temperature_Setpoint'],
  ['hot_water_return_temp_c', 'Hot Water Return Temperature Sensor', '°C', 'Hot_Water_Return_Temperature_Sensor'],
  ['chilled_water_supply_temp_c', 'Chilled Water Supply Temperature Sensor', '°C', 'Chilled_Water_Supply_Temperature_Sensor'],
  ['chilled_water_supply_setpoint_c', 'Chilled Water Supply Temperature Setpoint', '°C', 'Supply_Chilled_Water_Temperature_Setpoint'],
  ['chilled_water_return_temp_c', 'Chilled Water Return Temperature Sensor', '°C', 'Chilled_Water_Return_Temperature_Sensor'],
  ['hot_water_flow_kg_s', 'Hot Water Supply Flow Sensor', 'kg/s', 'Hot_Water_Flow_Sensor'],
  ['chilled_water_flow_kg_s', 'Chilled Water Supply Flow Sensor', 'kg/s', 'Chilled_Water_Flow_Sensor'],
  ['hot_water_actual_flow_kg_s', 'Hot Water Physical Flow · calculated', 'kg/s', 'Point'],
  ['chilled_water_actual_flow_kg_s', 'Chilled Water Physical Flow · calculated', 'kg/s', 'Point'],
  ['hot_water_bypass_flow_kg_s', 'Hot Water Bypass Flow · calculated', 'kg/s', 'Point'],
  ['chilled_water_bypass_flow_kg_s', 'Chilled Water Bypass Flow · calculated', 'kg/s', 'Point'],
  ['ahu_1_hot_water_flow_kg_s', 'AHU 1 Heating Coil Water Flow · calculated', 'kg/s', 'Point'],
  ['ahu_2_hot_water_flow_kg_s', 'AHU 2 Heating Coil Water Flow · calculated', 'kg/s', 'Point'],
  ['ahu_1_chilled_water_flow_kg_s', 'AHU 1 Cooling Coil Water Flow · calculated', 'kg/s', 'Point'],
  ['ahu_2_chilled_water_flow_kg_s', 'AHU 2 Cooling Coil Water Flow · calculated', 'kg/s', 'Point'],
  ['hot_water_reheat_flow_kg_s', 'VAV Reheat Water Flow · total calculated', 'kg/s', 'Point'],
  ['hot_water_differential_pressure_pa', 'Hot Water Remote Differential Pressure Sensor', 'Pa', 'Differential_Pressure_Sensor'],
  ['chilled_water_differential_pressure_pa', 'Chilled Water Remote Differential Pressure Sensor', 'Pa', 'Differential_Pressure_Sensor'],
  ['hot_water_pump_speed', 'Hot Water Pump Speed Command', 'fraction (0–1)', 'Speed_Command'],
  ['chilled_water_pump_speed', 'Chilled Water Pump Speed Command', 'fraction (0–1)', 'Speed_Command'],
  ['hot_water_pump_running', 'Hot Water Pump Running Status', 'Boolean (0/1)', 'Status'],
  ['chilled_water_pump_running', 'Chilled Water Pump Running Status', 'Boolean (0/1)', 'Status'],
  ['hot_water_enable', 'Hot Water System Enable Command', 'Boolean (0/1)', 'Hot_Water_System_Enable_Command'],
  ['chilled_water_enable', 'Chilled Water System Enable Command', 'Boolean (0/1)', 'Chilled_Water_System_Enable_Command'],
  ['water_pump_kw', 'Water Pump Electricity', 'kW'],
  ['electric_kw', 'Equipment electricity', 'kW'],
  ['gas_kw', 'Boiler fuel input', 'kW'],
]

export const physicalSignals: GraphSignal[] = [
  ...physicalFields.map(([field,label,unit,brickClass])=>({id:`physical|${field}`,label,kind:'physical' as const,type:'Real',unit,brickClass})),
  ...Array.from({length:10},(_,index)=>({id:`physical|zone_${String(index+1).padStart(2,'0')}_temp_c`,label:`Zone ${index+1} Air Temperature Sensor`,kind:'physical' as const,type:'Real',unit:'°C',brickClass:'Zone_Air_Temperature_Sensor'})),
  ...Array.from({length:10},(_,index)=>({id:`physical|zone_${String(index+1).padStart(2,'0')}_co2_ppm`,label:`Zone ${index+1} CO₂ Level Sensor`,kind:'physical' as const,type:'Real',unit:'ppm',brickClass:'Zone_CO2_Level_Sensor'})),
  ...Array.from({length:10},(_,index)=>({id:`physical|zone_${String(index+1).padStart(2,'0')}_presence_raw`,label:`Zone ${index+1} Raw Occupancy Sensor`,kind:'physical' as const,type:'Boolean',unit:'Boolean (0/1)',brickClass:'Occupancy_Sensor'})),
  ...Array.from({length:10},(_,index)=>({id:`physical|zone_${String(index+1).padStart(2,'0')}_occupant_count_actual`,label:`Zone ${index+1} Physical Occupants · modeled`,kind:'physical' as const,type:'Real',unit:'people',brickClass:'Point'})),
  ...Array.from({length:10},(_,index)=>({id:`physical|zone_${String(index+1).padStart(2,'0')}_airflow`,label:`Zone ${index+1} Primary Air Flow Sensor`,kind:'physical' as const,type:'Real',unit:'normalized airflow',brickClass:'Supply_Air_Flow_Sensor'})),
  ...Array.from({length:10},(_,index)=>({id:`physical|zone_${String(index+1).padStart(2,'0')}_discharge_c`,label:`Zone ${index+1} Discharge Air Temperature Sensor`,kind:'physical' as const,type:'Real',unit:'°C',brickClass:'Discharge_Air_Temperature_Sensor'})),
  ...Array.from({length:10},(_,index)=>({id:`physical|zone_${String(index+1).padStart(2,'0')}_actual_damper_position`,label:`Zone ${index+1} VAV Damper Position Sensor`,kind:'physical' as const,type:'Real',unit:'fraction (0–1)',brickClass:'Damper_Position_Sensor'})),
  ...Array.from({length:10},(_,index)=>({id:`physical|zone_${String(index+1).padStart(2,'0')}_actual_reheat_valve_position`,label:`Zone ${index+1} Reheat Valve Position Sensor`,kind:'physical' as const,type:'Real',unit:'fraction (0–1)',brickClass:'Valve_Position_Sensor'})),
]

export function physicalSignalsForGraph(graph: Block): GraphSignal[] {
  const available = graph.inputs?.some(port => port.name.endsWith('_co2_ppm'))
    ? physicalSignals : physicalSignals.filter(signal => !/^physical\|zone_\d{2}_co2_ppm$/.test(signal.id))
  const model = graph.brick_model
  if (!model) return available
  const entities = new Map(model.entities.map(entity => [entity.id, entity]))
  const roomByController = new Map<string, string>()
  for (const zone of model.entities.filter(entity => entity.class === 'brick:HVAC_Zone' && entity.controller)) {
    const roomId = model.relationships.find(relation => relation.subject === zone.id && relation.predicate === 'brick:hasPart')?.object
    const room = roomId ? entities.get(roomId) : undefined
    if (room?.class === 'rec:Room') roomByController.set(zone.controller!, room.label)
  }
  return available.map(signal => {
    const zone = signal.id.match(/^physical\|(zone_\d{2})_/)
    const room = zone && roomByController.get(zone[1])
    return room ? { ...signal, label: `${room} · ${signal.label}` } : signal
  })
}

export function signalValue(row:Row,id:string):number|undefined {
  if(!id.startsWith('physical|'))return row.traces?.[id]
  const field=id.slice('physical|'.length)
  const zone=field.match(/^zone_(\d{2})_temp_c$/)
  if(zone)return row.zone_c[Number(zone[1])-1]
  const co2=field.match(/^zone_(\d{2})_co2_ppm$/)
  if(co2)return row.zone_co2_ppm?.[Number(co2[1])-1]
  const occupancy=field.match(/^zone_(\d{2})_presence_raw$/)
  if(occupancy)return row.zone_presence_raw?.[Number(occupancy[1])-1]
  const occupants=field.match(/^zone_(\d{2})_occupant_count_actual$/)
  if(occupants)return row.zone_occupant_count_actual?.[Number(occupants[1])-1]
  const airflow=field.match(/^zone_(\d{2})_airflow$/)
  if(airflow)return row.zone_airflow?.[Number(airflow[1])-1]
  const discharge=field.match(/^zone_(\d{2})_discharge_c$/)
  if(discharge)return row.zone_discharge_c?.[Number(discharge[1])-1]
  const damper=field.match(/^zone_(\d{2})_actual_damper_position$/)
  if(damper)return row.zone_actual_damper_position?.[Number(damper[1])-1]
  const valve=field.match(/^zone_(\d{2})_actual_reheat_valve_position$/)
  if(valve)return row.zone_actual_reheat_valve_position?.[Number(valve[1])-1]
  const value=row[field as keyof Row]
  return typeof value==='number'?value:undefined
}

function unitFor(name:string,type:string,unit?:string):string {
  if(type==='Boolean')return 'Boolean (0/1)'
  if(type==='Integer')return unit?.trim()||'count'
  if(unit?.trim())return unit.trim()
  const key=name.toLowerCase()
  if(/error|deadband/.test(key))return 'K'
  if(/_kw|heat_pump_heating|heat_pump_cooling|boiler|chiller/.test(key))return 'kW'
  if(/fan_speed|fan_gain|fan_limit|fraction|damper|valve|demand|mode|enabled|cool_pi|heat_pi|reheat_select|pressure$|damper_offset|^one$/.test(key))return 'fraction (0–1)'
  if(/airflow|flow_request|flow_span|pressure_effect/.test(key))return 'normalized airflow'
  if(/temp|setpoint|threshold|supply|outdoor_c|mild_or_cool/.test(key))return '°C'
  return 'unit not declared'
}

function sourcePort(container:Block,source:string):{port?:Connector;name:string;type:string}{
  const [blockName,portName]=source.split('.')
  const child=container.blocks?.find(item=>item.name===blockName)
  const port=child?.outputs?.find(item=>item.name===portName)??container.inputs?.find(item=>item.name===source)
  return {port,name:child?`${child.name}.${portName}`:source,type:port?.type??'Real'}
}

export function graphSignals(graph: Block): GraphSignal[] {
  const signals: GraphSignal[] = []
  function visit(container: Block, path: string) {
    for (const port of container.inputs ?? []) {
      signals.push({ id: `${path}|${port.name}`, label: `${path} · ${port.label ?? port.name}`, kind: 'input', type: port.type, unit:unitFor(port.name,port.type,port.unit), brickClass:port.brick_class })
    }
    for (const port of container.outputs ?? []) {
      signals.push({ id: `${path}|output.${port.name}`, label: `${path} · ${port.label ?? port.name}`, kind: 'output', type: port.type, unit:unitFor(port.name,port.type,port.unit), brickClass:port.brick_class })
    }
    for (const child of container.blocks ?? []) {
      for (const port of child.outputs ?? []) {
        signals.push({ id: `${path}|${child.name}.${port.name}`, label: `${path}.${child.name} · ${port.label ?? port.name}`, kind: 'output', type: port.type, unit:unitFor(port.name==='y'?child.name:`${child.name}.${port.name}`,port.type,port.unit), brickClass:port.brick_class })
      }
    }
    for (const [index, edge] of (container.connections ?? []).entries()) {
      const [source, target] = edgeEndpoints(edge)
      const targetName = target.includes('.') ? target : `${container.name}.${target}`
      const {port,name,type}=sourcePort(container,source)
      signals.push({ id: `${path}|edge.${index}`, label: `${path}: ${port?.label ?? source} → ${targetName}`, kind: 'connection', type, unit:unitFor(name,type,port?.unit), brickClass:port?.brick_class })
    }
    for (const child of container.blocks ?? []) {
      if (child.blocks?.length) visit(child, `${path}.${child.name}`)
    }
  }
  visit(graph, graph.name)
  return signals
}
