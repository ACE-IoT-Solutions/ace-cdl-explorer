// Editorial help only. Interfaces and supported classes are generated from Rust.
export interface EditorialHelp { title?: string; description?: string; settings?: string; equations?: string[] }
const editorialHelp: Record<string, EditorialHelp> = {
  "Constant": {
    "title": "Constant",
    "description": "Outputs a fixed numeric value.",
    "settings": "k: value",
    "equations": [
      "y = k"
    ]
  },
  "Add": {
    "title": "Weighted add",
    "description": "Adds two inputs with optional separate gains.",
    "settings": "k1, k2: gains",
    "equations": [
      "y = k1·u1 + k2·u2"
    ]
  },
  "AddParameter": {
    "title": "Add parameter",
    "description": "Adds a fixed parameter to the input.",
    "settings": "p: offset",
    "equations": [
      "y = u + p"
    ]
  },
  "Subtract": {
    "title": "Subtract",
    "description": "Subtracts the second input from the first.",
    "equations": [
      "y = u1 − u2"
    ]
  },
  "Multiply": {
    "title": "Multiply",
    "description": "Multiplies two numeric inputs.",
    "equations": [
      "y = u1·u2"
    ]
  },
  "Divide": {
    "title": "Divide",
    "description": "Divides the first input by the second.",
    "equations": [
      "y = u1/u2"
    ]
  },
  "MultiplyByParameter": {
    "title": "Multiply by parameter",
    "description": "Scales a numeric input by a fixed gain.",
    "settings": "k: gain",
    "equations": [
      "y = k·u"
    ]
  },
  "Gain": {
    "title": "Gain",
    "description": "Alias of MultiplyByParameter; scales a numeric input by a fixed gain.",
    "settings": "k: gain",
    "equations": [
      "y = k·u"
    ]
  },
  "Min": {
    "title": "Minimum",
    "description": "Returns the smaller of two numeric inputs.",
    "equations": [
      "y = min(u1, u2)"
    ]
  },
  "Max": {
    "title": "Maximum",
    "description": "Returns the larger of two numeric inputs, including integer requests.",
    "equations": [
      "y = max(u1, u2)"
    ]
  },
  "Line": {
    "title": "Line",
    "description": "Interpolates between two editable support points, optionally clamping outside their input range.",
    "settings": "limitBelow, limitAbove: clamp input at x1 or x2",
    "equations": [
      "y = f1 + (f2 − f1)·(clip(u, x1, x2) − x1)/(x2 − x1)"
    ]
  },
  "BooleanToInteger": {
    "title": "Boolean to integer",
    "description": "Converts a Boolean input to configurable integer values.",
    "settings": "integerTrue, integerFalse: integer output values",
    "equations": [
      "y = u ? integerTrue : integerFalse"
    ]
  },
  "RealToInteger": {
    "title": "Real to integer",
    "description": "Rounds a real signal to the nearest integer, with half values rounded away from zero.",
    "equations": [
      "y = u > 0 ? floor(u + 0.5) : ceil(u − 0.5)"
    ]
  },
  "BooleanToReal": {
    "title": "Boolean to real",
    "description": "Converts a Boolean condition to configurable real values.",
    "equations": [
      "y = u ? realTrue : realFalse"
    ]
  },
  "IntegerToReal": {
    "title": "Integer to real",
    "description": "Converts an integer request count into a real number for arithmetic.",
    "equations": [
      "y = real(u)"
    ]
  },
  "GreaterThreshold": {
    "title": "Greater than threshold",
    "description": "Returns true above a threshold. Real input can retain true until it falls to t minus the hysteresis h.",
    "settings": "t: threshold; h: optional Real hysteresis",
    "equations": [
      "h = 0: y = (u > t)",
      "h > 0: y rises above t and clears at u ≤ t − h"
    ]
  },
  "LessThreshold": {
    "title": "Less than threshold",
    "description": "Returns true below a threshold and retains true until the input reaches t plus the hysteresis h.",
    "settings": "t: threshold; h: optional Real hysteresis",
    "equations": [
      "h = 0: y = (u < t)",
      "h > 0: y rises below t and clears at u ≥ t + h"
    ]
  },
  "Equal": {
    "title": "Integer equal",
    "description": "Returns true when two integer inputs are equal.",
    "equations": [
      "y = (u1 = u2)"
    ]
  },
  "Limiter": {
    "title": "Limiter",
    "description": "Clamps the input to a configured lower and upper bound.",
    "settings": "uMin, uMax: bounds",
    "equations": [
      "y = min(uMax, max(uMin, u))"
    ]
  },
  "LimPID": {
    "title": "Limited PI controller",
    "description": "Combines proportional error and accumulated error, then limits the command. The runtime implements the PI part of this CDL block.",
    "settings": "k: gain; Ti: integral time; yMin/yMax: limits; reverseActing: direction",
    "equations": [
      "y = clip(k·e + I_next, yMin, yMax)",
      "e = reverseActing ? (u_s − u_m) : (u_m − u_s)",
      "I_next = clip(I + k·e·Δt/Ti, yMin − k·e, yMax − k·e)"
    ]
  },
  "PIDWithReset": {
    "title": "PID with output reset",
    "description": "Selectable P, PI, PD, or PID control. A rising trigger resets the integral term when present; derivative action uses an OBC first-order filter.",
    "settings": "controllerType: P/PI/PD/PID; k: gain; Ti/Td: integral/derivative time; r: error scale; Ni/Nd: anti-windup/derivative filter; yMin/yMax: limits; y_reset: reset output",
    "equations": [
      "e = (reverseActing ? u_s − u_m : u_m − u_s)/r",
      "P = k·e; D = k·Td·(e − x)/(Td/Nd), with dx/dt = (e − x)/(Td/Nd)",
      "on trigger rising (PI/PID): I = y_reset − P − D",
      "otherwise (PI/PID): I_next = I + k·Δt/Ti·(e − (y_raw − clip(y_raw))/(k·Ni))",
      "y = clip(P + (PI/PID ? I : 0) + (PD/PID ? D : 0), yMin, yMax)"
    ]
  },
  "Filter": {
    "title": "First-order filter",
    "description": "Smooths changes using the prior output and a time constant.",
    "settings": "T: time constant in seconds",
    "equations": [
      "y = y_prev + (1 − exp(−Δt/T))·(u − y_prev)"
    ]
  },
  "LimRateLimiter": {
    "title": "Rate limiter",
    "description": "Limits how much a numeric output may rise or fall per second.",
    "settings": "riseRate, fallRate: units per second",
    "equations": [
      "y = clip(u, y_prev − fallRate·Δt, y_prev + riseRate·Δt)"
    ]
  },
  "Hysteresis": {
    "title": "Hysteresis",
    "description": "Changes a Boolean output at lower and upper thresholds and remembers its state between them.",
    "settings": "uLow, uHigh: thresholds",
    "equations": [
      "y = true if u > uHigh; false if u < uLow; else y_prev"
    ]
  },
  "Switch": {
    "title": "Legacy switch",
    "description": "Chooses u1 when the Boolean selector is true and u2 when false. Imported CDL variants may use Boolean u2 as selector and u3 as the false value.",
    "equations": [
      "y = u3 ? u1 : u2"
    ]
  },
  "Greater": {
    "title": "Greater than",
    "description": "Returns true above u2; optional hysteresis h keeps a true output until u1 reaches u2 − h.",
    "settings": "h: hysteresis; pre_y_start: initial Boolean state",
    "equations": [
      "h = 0: y = (u1 > u2)",
      "h > 0: y rises when u1 > u2 and clears when u1 ≤ u2 − h"
    ]
  },
  "Less": {
    "title": "Less than",
    "description": "Returns true below u2; optional hysteresis h keeps a true output until u1 reaches u2 + h.",
    "settings": "h: hysteresis; pre_y_start: initial Boolean state",
    "equations": [
      "h = 0: y = (u1 < u2)",
      "h > 0: y rises when u1 < u2 and clears when u1 ≥ u2 + h"
    ]
  },
  "And": {
    "title": "Boolean AND",
    "description": "Returns true only when both inputs are true.",
    "equations": [
      "y = u1 ∧ u2"
    ]
  },
  "Or": {
    "title": "Boolean OR",
    "description": "Returns true when either input is true.",
    "equations": [
      "y = u1 ∨ u2"
    ]
  },
  "Not": {
    "title": "Boolean NOT",
    "description": "Inverts a Boolean input.",
    "equations": [
      "y = ¬u"
    ]
  },
  "Delay": {
    "title": "One-step delay",
    "description": "Returns the value held from the previous simulation step.",
    "equations": [
      "y[n] = u[n−1]"
    ]
  },
  "UnitDelay": {
    "title": "Sampled unit delay",
    "description": "Returns the previous sampled input, allowing feedback through an explicit state block.",
    "settings": "samplePeriod: seconds; y_start: initial output",
    "equations": [
      "y[k] = u[k−1] at each sample; hold between samples"
    ]
  },
  "Sampler": {
    "title": "Periodic sampler",
    "description": "Samples a real input at its configured period and holds the result between samples.",
    "settings": "samplePeriod: seconds",
    "equations": [
      "y[k] = u at t = k × samplePeriod; hold between samples"
    ]
  },
  "Pre": {
    "title": "Previous Boolean iteration",
    "description": "Returns the Boolean input from the previous event iteration; the event settles when input and output agree.",
    "settings": "pre_u_start: initial input",
    "equations": [
      "y = pre(u); repeat event evaluation until u = y"
    ]
  },
  "TriggeredSampler": {
    "title": "Triggered sampler",
    "description": "Samples a real input when the trigger rises and holds the value.",
    "settings": "y_start: initial output",
    "equations": [
      "y = u on rising(trigger); otherwise hold y_prev"
    ]
  },
  "TriggeredMovingMean": {
    "title": "Triggered moving mean",
    "description": "Averages the latest n triggered samples. Samples once at initialization.",
    "settings": "n: number of samples",
    "equations": [
      "y = mean(last n samples of u on rising(trigger))"
    ]
  },
  "SampleTrigger": {
    "title": "Periodic sample trigger",
    "description": "Pulses at fixed clock times, such as midnight for daily optimal-start learning.",
    "settings": "period: seconds; shift: seconds",
    "equations": [
      "y = true at t = shift + k × period; false between samples"
    ]
  },
  "Edge": {
    "title": "Rising edge",
    "description": "Pulses true for one step when the Boolean input rises.",
    "settings": "pre_u_start: initial input state",
    "equations": [
      "y = u ∧ ¬u_prev"
    ]
  },
  "FallingEdge": {
    "title": "Falling edge",
    "description": "Pulses true for one step when the Boolean input falls.",
    "settings": "pre_u_start: initial input state",
    "equations": [
      "y = ¬u ∧ u_prev"
    ]
  },
  "Timer": {
    "title": "Timer",
    "description": "Reports elapsed time while the Boolean input remains true and whether the threshold has passed.",
    "settings": "t: threshold in seconds",
    "equations": [
      "y = u ? elapsed since rising(u) : 0",
      "passed = u ∧ (y ≥ t)"
    ]
  },
  "TrueDelay": {
    "title": "True delay",
    "description": "Waits for an input to stay true for the configured time; a false input clears immediately.",
    "settings": "delayTime: seconds; delayOnInit: delay an initially true input",
    "equations": [
      "y = u ∧ (continuous true time ≥ delayTime)"
    ]
  },
  "TrueFalseHold": {
    "title": "True/false hold",
    "description": "Holds each output state for its configured minimum duration before accepting the opposite input.",
    "settings": "trueHoldDuration, falseHoldDuration: seconds",
    "equations": [
      "y = u when the current output hold has expired; otherwise hold y_prev"
    ]
  },
  "Latch": {
    "title": "Boolean latch",
    "description": "Keeps a rising set signal true until a rising clear signal resets it.",
    "equations": [
      "on rising(u) or rising(clr): y = u ∧ ¬clr; otherwise hold y"
    ]
  },
  "Composite": {
    "title": "Composite",
    "description": "Contains connected child blocks and exposes chosen boundary ports. Its behavior comes from those children."
  },
  "VAVBoxController": {
    "title": "VAV box controller",
    "description": "Uses zone and supply air temperature to command a damper and reheat valve; calculates a normalized airflow request and diagnostic demands."
  },
  "AHUController": {
    "title": "AHU controller",
    "description": "Chooses a supply air temperature setpoint from outdoor temperature and a supply fan speed command from the previous airflow request."
  },
  "PlantDispatch": {
    "title": "Plant dispatch",
    "description": "Splits requested heating and cooling between heat pump and backup boiler/chiller using an editable CDL dispatch fraction."
  },
  "BuildingController": {
    "title": "Building controller",
    "description": "Connects the plant, two AHUs, and ten VAV controllers in the bundled four-pipe example."
  },
  "Buildings.Controls.OBC.CDL.Reals.Add": {
    "title": "Real add",
    "description": "Adds two real inputs.",
    "equations": [
      "y = u1 + u2"
    ]
  },
  "Buildings.Controls.OBC.CDL.Integers.Add": {
    "title": "Integer add",
    "description": "Adds two integer inputs.",
    "equations": [
      "y = u1 + u2"
    ]
  },
  "Buildings.Controls.OBC.CDL.Reals.Switch": {
    "description": "Selects u1 or u3 using Boolean input u2.",
    "equations": [
      "y = u2 ? u1 : u3"
    ]
  },
  "Buildings.Controls.OBC.CDL.Integers.Switch": {
    "description": "Selects u1 or u3 using Boolean input u2.",
    "equations": [
      "y = u2 ? u1 : u3"
    ]
  },
  "Buildings.Controls.OBC.CDL.Logical.Switch": {
    "description": "Selects u1 or u3 using Boolean input u2.",
    "equations": [
      "y = u2 ? u1 : u3"
    ]
  }
}
editorialHelp['Buildings.Controls.OBC.CDL.Logical.Sources.Constant'] = {description:'Outputs a fixed Boolean value.', equations:['y = k']}
export function helpForType(type: string): EditorialHelp { return editorialHelp[type] ?? editorialHelp[type.split(".").at(-1) ?? type] ?? {} }
