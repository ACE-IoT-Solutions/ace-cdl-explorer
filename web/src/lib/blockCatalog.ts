export interface BlockDefinition {
  type: string
  title: string
  group: 'Elementary blocks' | 'Composite building sequences'
  description: string
  inputs: string
  outputs: string
  settings?: string
}

// Keep this list in sync with runtime/src/lib.rs::known_type and the editor palette.
// The four building sequences are composites in the bundled example, not Brick classes.
export const blockCatalog: BlockDefinition[] = [
  {type:'Constant',title:'Constant',group:'Elementary blocks',description:'Outputs a fixed numeric value.',inputs:'None',outputs:'y: Real',settings:'k: value'},
  {type:'Add',title:'Add',group:'Elementary blocks',description:'Adds two inputs with optional separate gains: y = k1·u1 + k2·u2.',inputs:'u1, u2: Real',outputs:'y: Real',settings:'k1, k2: gains'},
  {type:'Subtract',title:'Subtract',group:'Elementary blocks',description:'Subtracts the second input from the first.',inputs:'u1, u2: Real',outputs:'y: Real'},
  {type:'Multiply',title:'Multiply',group:'Elementary blocks',description:'Multiplies two numeric inputs.',inputs:'u1, u2: Real',outputs:'y: Real'},
  {type:'MultiplyByParameter',title:'Multiply by parameter',group:'Elementary blocks',description:'Scales a numeric input by a fixed gain.',inputs:'u: Real',outputs:'y: Real',settings:'k: gain'},
  {type:'Gain',title:'Gain',group:'Elementary blocks',description:'Alias of MultiplyByParameter; scales a numeric input by a fixed gain.',inputs:'u: Real',outputs:'y: Real',settings:'k: gain'},
  {type:'Min',title:'Minimum',group:'Elementary blocks',description:'Returns the smaller of two numeric inputs.',inputs:'u1, u2: Real',outputs:'y: Real'},
  {type:'Limiter',title:'Limiter',group:'Elementary blocks',description:'Clamps the input to a configured lower and upper bound.',inputs:'u: Real',outputs:'y: Real',settings:'uMin, uMax: bounds'},
  {type:'LimPID',title:'Limited PI controller',group:'Elementary blocks',description:'Combines proportional error and accumulated error, then limits the command. The runtime implements the PI part of this CDL block.',inputs:'u_s: setpoint; u_m: measurement',outputs:'y: Real',settings:'k: gain; Ti: integral time; yMin/yMax: limits; reverseActing: direction'},
  {type:'Filter',title:'First-order filter',group:'Elementary blocks',description:'Smooths changes using the prior output and a time constant.',inputs:'u: Real',outputs:'y: Real',settings:'T: time constant in seconds'},
  {type:'LimRateLimiter',title:'Rate limiter',group:'Elementary blocks',description:'Limits how much a numeric output may rise or fall per second.',inputs:'u: Real',outputs:'y: Real',settings:'riseRate, fallRate: units per second'},
  {type:'Hysteresis',title:'Hysteresis',group:'Elementary blocks',description:'Changes a Boolean output at lower and upper thresholds and remembers its state between them.',inputs:'u: Real',outputs:'y: Boolean',settings:'uLow, uHigh: thresholds'},
  {type:'Switch',title:'Switch',group:'Elementary blocks',description:'Chooses u1 when the Boolean selector is true and u2 when false.',inputs:'u1, u2: Real; u3: Boolean',outputs:'y: Real'},
  {type:'Greater',title:'Greater than',group:'Elementary blocks',description:'Returns true when u1 is greater than u2.',inputs:'u1, u2: Real',outputs:'y: Boolean'},
  {type:'Less',title:'Less than',group:'Elementary blocks',description:'Returns true when u1 is less than u2.',inputs:'u1, u2: Real',outputs:'y: Boolean'},
  {type:'And',title:'Boolean AND',group:'Elementary blocks',description:'Returns true only when both inputs are true.',inputs:'u1, u2: Boolean',outputs:'y: Boolean'},
  {type:'Or',title:'Boolean OR',group:'Elementary blocks',description:'Returns true when either input is true.',inputs:'u1, u2: Boolean',outputs:'y: Boolean'},
  {type:'Not',title:'Boolean NOT',group:'Elementary blocks',description:'Inverts a Boolean input.',inputs:'u: Boolean',outputs:'y: Boolean'},
  {type:'Delay',title:'One-step delay',group:'Elementary blocks',description:'Returns the value held from the previous simulation step.',inputs:'u: Real',outputs:'y: Real'},
  {type:'Composite',title:'Composite',group:'Composite building sequences',description:'Contains connected child blocks and exposes chosen boundary ports. Its behavior comes from those children.',inputs:'User-defined',outputs:'User-defined'},
  {type:'VAVBoxController',title:'VAV box controller',group:'Composite building sequences',description:'Uses zone and supply air temperature to command a damper and reheat valve; calculates a normalized airflow request and diagnostic demands.',inputs:'Zone temperature, supply temperature, normalized pressure',outputs:'Damper, reheat, airflow, cooling and heating demand'},
  {type:'AHUController',title:'AHU controller',group:'Composite building sequences',description:'Chooses a supply air temperature setpoint from outdoor temperature and a supply fan speed command from the previous airflow request.',inputs:'Outdoor temperature, airflow request',outputs:'Supply temperature setpoint, fan speed command'},
  {type:'PlantDispatch',title:'Plant dispatch',group:'Composite building sequences',description:'Splits requested heating and cooling between heat pump and backup boiler/chiller according to the heat pump fraction.',inputs:'Heating and cooling requests, heat pump fraction',outputs:'Heat pump heating/cooling, boiler and chiller commands'},
  {type:'BuildingController',title:'Building controller',group:'Composite building sequences',description:'Connects the plant, two AHUs, and ten VAV controllers in the bundled four-pipe example.',inputs:'Outdoor, zone and plant feedback',outputs:'Equipment commands and zone control requests'},
]
