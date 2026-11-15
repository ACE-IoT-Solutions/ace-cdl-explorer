import type { Block, Connector } from './types'
import { helpForType } from './blockHelpData'

const portDetails: Record<string, Record<string, string>> = {
  VAVBoxController: {
    'input.room_temp': 'Measured zone temperature',
    'input.supply_air_temp': 'Air temperature delivered by the AHU',
    'input.supply_pressure': 'Normalized duct pressure',
    'output.damper_position': 'VAV damper command, 0–1',
    'output.reheat_valve_position': 'Hot water reheat valve command, 0–1',
    'output.airflow': 'Estimated zone airflow',
    'output.cooling_demand': 'Normalized zone cooling request, 0–1',
    'output.heating_demand': 'Normalized zone heating request, 0–1',
  },
  AHUController: {
    'input.outdoor_c': 'Outdoor air temperature in °C',
    'input.flow_request': 'Requested airflow from the previous step',
    'output.supply_air_temp': 'Supply air temperature setpoint in °C',
    'output.fan_speed': 'Normalized supply fan command, 0–1',
  },
  PlantDispatch: {
    'input.heat_request_kw': 'Heating request from the previous step, kW',
    'input.cool_request_kw': 'Cooling request from the previous step, kW',
    'output.heat_pump_heating_kw': 'Heat pump heating command, kW',
    'output.heat_pump_cooling_kw': 'Heat pump cooling command, kW',
    'output.boiler_kw': 'Boiler heating command, kW',
    'output.chiller_kw': 'Chiller cooling command, kW',
  },
  LimPID: { 'input.u_s': 'Control setpoint', 'input.u_m': 'Measured value', 'output.y': 'Bounded PI control command' },
  PIDWithReset: { 'input.u_s': 'Control setpoint', 'input.u_m': 'Measured value', 'input.trigger': 'Reset output on a rising edge', 'output.y': 'Bounded PI control command' },
  Switch: { 'input.u1': 'Value used when u3 is true', 'input.u2': 'Value used when u3 is false', 'input.u3': 'Boolean selector', 'output.y': 'Selected value' },
  Add: { 'input.u1': 'First term', 'input.u2': 'Second term', 'output.y': 'Weighted sum' },
  AddParameter: { 'input.u': 'Value to offset', 'output.y': 'Input plus the fixed parameter p' },
  Subtract: { 'input.u1': 'Value to subtract from', 'input.u2': 'Value to subtract', 'output.y': 'Difference, u1 − u2' },
  Multiply: { 'input.u1': 'First factor', 'input.u2': 'Second factor', 'output.y': 'Product' },
  Divide: { 'input.u1': 'Numerator', 'input.u2': 'Denominator', 'output.y': 'Quotient' },
  Min: { 'input.u1': 'First value', 'input.u2': 'Second value', 'output.y': 'Smaller value' },
  Max: { 'input.u1': 'First value', 'input.u2': 'Second value', 'output.y': 'Larger value' },
  BooleanToInteger: { 'input.u': 'Boolean condition', 'output.y': 'Configured integer value' },
  RealToInteger: { 'input.u': 'Real value', 'output.y': 'Nearest integer' },
  BooleanToReal: { 'input.u': 'Boolean condition', 'output.y': 'Configured real value' },
  GreaterThreshold: { 'input.u': 'Integer or Real value to compare', 'output.y': 'True above t; optional Real hysteresis h' },
  LessThreshold: { 'input.u': 'Real value to compare', 'output.y': 'True below t; clears at t plus h' },
  Equal: { 'input.u1': 'First integer', 'input.u2': 'Second integer', 'output.y': 'True when u1 = u2' },
  Greater: { 'input.u1': 'First value', 'input.u2': 'Second value', 'output.y': 'True if u1 > u2' },
  Less: { 'input.u1': 'First value', 'input.u2': 'Second value', 'output.y': 'True if u1 < u2' },
  Hysteresis: { 'input.u': 'Value compared with both thresholds', 'output.y': 'Remembered Boolean state' },
  Filter: { 'input.u': 'Raw signal', 'output.y': 'Smoothed signal' },
  Limiter: { 'input.u': 'Signal to constrain', 'output.y': 'Bounded signal' },
  Constant: { 'output.y': 'Configured value k' },
  MultiplyByParameter: { 'input.u': 'Signal to scale', 'output.y': 'Input multiplied by k' },
  Delay: { 'input.u': 'Current value', 'output.y': 'Previous step value' },
  TriggeredSampler: { 'input.u': 'Value to sample', 'input.trigger': 'Sample on a rising edge', 'output.y': 'Held sampled value' },
  TriggeredMovingMean: { 'input.u': 'Value to add to the moving window', 'input.trigger': 'Add sample on a rising edge', 'output.y': 'Mean of the latest n samples' },
  SampleTrigger: { 'output.y': 'Pulse at each configured period and shift' },
  Edge: { 'input.u': 'Boolean input', 'output.y': 'One-step pulse on a rising edge' },
  FallingEdge: { 'input.u': 'Boolean input', 'output.y': 'One-step pulse on a falling edge' },
  Timer: { 'input.u': 'Condition to time', 'output.y': 'Elapsed seconds while true', 'output.passed': 'True after t seconds' },
  TrueDelay: { 'input.u': 'Raw Boolean condition', 'output.y': 'True after the configured continuous delay' },
  TrueFalseHold: { 'input.u': 'Requested Boolean state', 'output.y': 'State after the minimum hold has elapsed' },
  Latch: { 'input.u': 'Set request', 'input.clr': 'Clear request', 'output.y': 'Remembered Boolean state' },
}

export function blockKind(block: Block): string {
  return block.block_type.split('.').at(-1) ?? block.block_type
}

export function blockEquations(block:Block):string[] {
  const declared=(block.equations??[]).filter(equation=>equation.lhs?.trim()&&equation.rhs?.trim())
  if(declared.length)return declared.map(equation=>`${equation.lhs.trim()} = ${equation.rhs.trim()}`)
  const kind=blockKind(block)
  if(kind==='Switch'&&block.inputs?.some(port=>port.name==='u2'&&port.type==='Boolean'))return ['y = u2 ? u1 : u3']
  return helpForType(block.block_type).equations??[]
}

export function blockDescription(block: Block): string {
  return block.description?.trim() || helpForType(block.block_type).description || 'Executes this block’s declared input, output, and parameter behavior.'
}

export function portDescription(block: Block, direction: 'input' | 'output', port: Connector): string {
  return port.description?.trim() || portDetails[blockKind(block)]?.[`${direction}.${port.name}`]
    || (direction === 'input' ? 'Input signal' : 'Output signal')
}
