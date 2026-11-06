export interface BlockDefinition {
  type: string
  title: string
  group: 'Elementary blocks' | 'Composite building sequences'
  description: string
  inputs: string
  outputs: string
  settings?: string
  equations?: string[]
}

// Keep this list in sync with runtime/src/lib.rs::known_type and the editor palette.
// The four building sequences are composites in the bundled example, not Brick classes.
export const blockCatalog: BlockDefinition[] = [
  {type:'Constant',title:'Constant',group:'Elementary blocks',description:'Outputs a fixed numeric value.',inputs:'None',outputs:'y: Real',settings:'k: value',equations:['y = k']},
  {type:'Add',title:'Add',group:'Elementary blocks',description:'Adds two inputs with optional separate gains.',inputs:'u1, u2: Real',outputs:'y: Real',settings:'k1, k2: gains',equations:['y = k1·u1 + k2·u2']},
  {type:'Subtract',title:'Subtract',group:'Elementary blocks',description:'Subtracts the second input from the first.',inputs:'u1, u2: Real',outputs:'y: Real',equations:['y = u1 − u2']},
  {type:'Multiply',title:'Multiply',group:'Elementary blocks',description:'Multiplies two numeric inputs.',inputs:'u1, u2: Real',outputs:'y: Real',equations:['y = u1·u2']},
  {type:'MultiplyByParameter',title:'Multiply by parameter',group:'Elementary blocks',description:'Scales a numeric input by a fixed gain.',inputs:'u: Real',outputs:'y: Real',settings:'k: gain',equations:['y = k·u']},
  {type:'Gain',title:'Gain',group:'Elementary blocks',description:'Alias of MultiplyByParameter; scales a numeric input by a fixed gain.',inputs:'u: Real',outputs:'y: Real',settings:'k: gain',equations:['y = k·u']},
  {type:'Min',title:'Minimum',group:'Elementary blocks',description:'Returns the smaller of two numeric inputs.',inputs:'u1, u2: Real',outputs:'y: Real',equations:['y = min(u1, u2)']},
  {type:'Max',title:'Maximum',group:'Elementary blocks',description:'Returns the larger of two numeric inputs, including integer requests.',inputs:'u1, u2: Real or Integer',outputs:'y: Real or Integer',equations:['y = max(u1, u2)']},
  {type:'BooleanToInteger',title:'Boolean to integer',group:'Elementary blocks',description:'Converts a Boolean condition into an integer request.',inputs:'u: Boolean',outputs:'y: Integer',equations:['y = u ? 1 : 0']},
  {type:'IntegerToReal',title:'Integer to real',group:'Elementary blocks',description:'Converts an integer request count into a real number for arithmetic.',inputs:'u: Integer',outputs:'y: Real',equations:['y = real(u)']},
  {type:'Limiter',title:'Limiter',group:'Elementary blocks',description:'Clamps the input to a configured lower and upper bound.',inputs:'u: Real',outputs:'y: Real',settings:'uMin, uMax: bounds',equations:['y = min(uMax, max(uMin, u))']},
  {type:'LimPID',title:'Limited PI controller',group:'Elementary blocks',description:'Combines proportional error and accumulated error, then limits the command. The runtime implements the PI part of this CDL block.',inputs:'u_s: setpoint; u_m: measurement',outputs:'y: Real',settings:'k: gain; Ti: integral time; yMin/yMax: limits; reverseActing: direction',equations:['y = clip(k·e + I_next, yMin, yMax)','e = reverseActing ? (u_s − u_m) : (u_m − u_s)','I_next = clip(I + k·e·Δt/Ti, yMin − k·e, yMax − k·e)']},
  {type:'Filter',title:'First-order filter',group:'Elementary blocks',description:'Smooths changes using the prior output and a time constant.',inputs:'u: Real',outputs:'y: Real',settings:'T: time constant in seconds',equations:['y = y_prev + (1 − exp(−Δt/T))·(u − y_prev)']},
  {type:'LimRateLimiter',title:'Rate limiter',group:'Elementary blocks',description:'Limits how much a numeric output may rise or fall per second.',inputs:'u: Real',outputs:'y: Real',settings:'riseRate, fallRate: units per second',equations:['y = clip(u, y_prev − fallRate·Δt, y_prev + riseRate·Δt)']},
  {type:'Hysteresis',title:'Hysteresis',group:'Elementary blocks',description:'Changes a Boolean output at lower and upper thresholds and remembers its state between them.',inputs:'u: Real',outputs:'y: Boolean',settings:'uLow, uHigh: thresholds',equations:['y = true if u > uHigh; false if u < uLow; else y_prev']},
  {type:'Switch',title:'Switch',group:'Elementary blocks',description:'Chooses u1 when the Boolean selector is true and u2 when false. Imported CDL variants may use Boolean u2 as selector and u3 as the false value.',inputs:'u1, u2: Real; u3: Boolean',outputs:'y: Real',equations:['y = u3 ? u1 : u2']},
  {type:'Greater',title:'Greater than',group:'Elementary blocks',description:'Returns true when u1 is greater than u2.',inputs:'u1, u2: Real',outputs:'y: Boolean',equations:['y = (u1 > u2)']},
  {type:'Less',title:'Less than',group:'Elementary blocks',description:'Returns true when u1 is less than u2.',inputs:'u1, u2: Real',outputs:'y: Boolean',equations:['y = (u1 < u2)']},
  {type:'And',title:'Boolean AND',group:'Elementary blocks',description:'Returns true only when both inputs are true.',inputs:'u1, u2: Boolean',outputs:'y: Boolean',equations:['y = u1 ∧ u2']},
  {type:'Or',title:'Boolean OR',group:'Elementary blocks',description:'Returns true when either input is true.',inputs:'u1, u2: Boolean',outputs:'y: Boolean',equations:['y = u1 ∨ u2']},
  {type:'Not',title:'Boolean NOT',group:'Elementary blocks',description:'Inverts a Boolean input.',inputs:'u: Boolean',outputs:'y: Boolean',equations:['y = ¬u']},
  {type:'Delay',title:'One-step delay',group:'Elementary blocks',description:'Returns the value held from the previous simulation step.',inputs:'u: Real',outputs:'y: Real',equations:['y[n] = u[n−1]']},
  {type:'UnitDelay',title:'Sampled unit delay',group:'Elementary blocks',description:'Returns the previous sampled input, allowing feedback through an explicit state block.',inputs:'u: Real',outputs:'y: Real',settings:'samplePeriod: seconds; y_start: initial output',equations:['y[k] = u[k−1] at each sample; hold between samples']},
  {type:'TrueDelay',title:'True delay',group:'Elementary blocks',description:'Waits for an input to stay true for the configured time; a false input clears immediately.',inputs:'u: Boolean',outputs:'y: Boolean',settings:'delayTime: seconds; delayOnInit: delay an initially true input',equations:['y = u ∧ (continuous true time ≥ delayTime)']},
  {type:'Latch',title:'Boolean latch',group:'Elementary blocks',description:'Keeps a rising set signal true until a rising clear signal resets it.',inputs:'u: Boolean set; clr: Boolean clear',outputs:'y: Boolean',equations:['on rising(u) or rising(clr): y = u ∧ ¬clr; otherwise hold y']},
  {type:'Composite',title:'Composite',group:'Composite building sequences',description:'Contains connected child blocks and exposes chosen boundary ports. Its behavior comes from those children.',inputs:'User-defined',outputs:'User-defined'},
  {type:'VAVBoxController',title:'VAV box controller',group:'Composite building sequences',description:'Uses zone and supply air temperature to command a damper and reheat valve; calculates a normalized airflow request and diagnostic demands.',inputs:'Zone temperature, supply temperature, normalized pressure',outputs:'Damper, reheat, airflow, cooling and heating demand'},
  {type:'AHUController',title:'AHU controller',group:'Composite building sequences',description:'Chooses a supply air temperature setpoint from outdoor temperature and a supply fan speed command from the previous airflow request.',inputs:'Outdoor temperature, airflow request',outputs:'Supply temperature setpoint, fan speed command'},
  {type:'PlantDispatch',title:'Plant dispatch',group:'Composite building sequences',description:'Splits requested heating and cooling between heat pump and backup boiler/chiller according to the heat pump fraction.',inputs:'Heating and cooling requests, heat pump fraction',outputs:'Heat pump heating/cooling, boiler and chiller commands'},
  {type:'BuildingController',title:'Building controller',group:'Composite building sequences',description:'Connects the plant, two AHUs, and ten VAV controllers in the bundled four-pipe example.',inputs:'Outdoor, zone and plant feedback',outputs:'Equipment commands and zone control requests'},
]
