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
  ['hot_water_differential_pressure_pa', 'Hot Water Remote Differential Pressure Sensor', 'Pa', 'Differential_Pressure_Sensor'],
  ['chilled_water_differential_pressure_pa', 'Chilled Water Remote Differential Pressure Sensor', 'Pa', 'Differential_Pressure_Sensor'],
  ['hot_water_pump_speed', 'Hot Water Pump Speed Command', 'fraction (0–1)', 'Speed_Command'],
  ['chilled_water_pump_speed', 'Chilled Water Pump Speed Command', 'fraction (0–1)', 'Speed_Command'],
  ['hot_water_enable', 'Hot Water System Enable Command', 'Boolean (0/1)', 'Hot_Water_System_Enable_Command'],
  ['chilled_water_enable', 'Chilled Water System Enable Command', 'Boolean (0/1)', 'Chilled_Water_System_Enable_Command'],
  ['water_pump_kw', 'Water Pump Electricity', 'kW'],
  ['electric_kw', 'Equipment electricity', 'kW'],
  ['gas_kw', 'Boiler fuel input', 'kW'],
]

export const physicalSignals: GraphSignal[] = [
  ...physicalFields.map(([field,label,unit,brickClass])=>({id:`physical|${field}`,label,kind:'physical' as const,type:'Real',unit,brickClass})),
  ...Array.from({length:10},(_,index)=>({id:`physical|zone_${String(index+1).padStart(2,'0')}_temp_c`,label:`Zone ${index+1} Air Temperature Sensor`,kind:'physical' as const,type:'Real',unit:'°C',brickClass:'Zone_Air_Temperature_Sensor'})),
  ...Array.from({length:10},(_,index)=>({id:`physical|zone_${String(index+1).padStart(2,'0')}_presence_raw`,label:`Zone ${index+1} Raw Occupancy Sensor`,kind:'physical' as const,type:'Boolean',unit:'Boolean (0/1)',brickClass:'Occupancy_Sensor'})),
  ...Array.from({length:10},(_,index)=>({id:`physical|zone_${String(index+1).padStart(2,'0')}_airflow`,label:`Zone ${index+1} Primary Air Flow Sensor`,kind:'physical' as const,type:'Real',unit:'normalized airflow',brickClass:'Supply_Air_Flow_Sensor'})),
  ...Array.from({length:10},(_,index)=>({id:`physical|zone_${String(index+1).padStart(2,'0')}_discharge_c`,label:`Zone ${index+1} Discharge Air Temperature Sensor`,kind:'physical' as const,type:'Real',unit:'°C',brickClass:'Discharge_Air_Temperature_Sensor'})),
]

export function signalValue(row:Row,id:string):number|undefined {
  if(!id.startsWith('physical|'))return row.traces?.[id]
  const field=id.slice('physical|'.length)
  const zone=field.match(/^zone_(\d{2})_temp_c$/)
  if(zone)return row.zone_c[Number(zone[1])-1]
  const occupancy=field.match(/^zone_(\d{2})_presence_raw$/)
  if(occupancy)return row.zone_presence_raw?.[Number(occupancy[1])-1]
  const airflow=field.match(/^zone_(\d{2})_airflow$/)
  if(airflow)return row.zone_airflow?.[Number(airflow[1])-1]
  const discharge=field.match(/^zone_(\d{2})_discharge_c$/)
  if(discharge)return row.zone_discharge_c?.[Number(discharge[1])-1]
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
