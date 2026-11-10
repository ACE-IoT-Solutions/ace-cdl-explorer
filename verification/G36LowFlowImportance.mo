within;
model G36LowFlowImportance
  "Compare the pinned OBC reheat low-flow alarm with pressure-reset importance on and off"
  Buildings.Controls.OBC.ASHRAE.G36.TerminalUnits.Reheat.Subsequences.Alarms active(
    staPreMul=1, hotWatRes=1, VCooMax_flow=0.95, floHys=0.05,
    damPosHys=0.05, valPosHys=0.05);
  Buildings.Controls.OBC.ASHRAE.G36.TerminalUnits.Reheat.Subsequences.Alarms suppressed(
    staPreMul=0, hotWatRes=1, VCooMax_flow=0.95, floHys=0.05,
    damPosHys=0.05, valPosHys=0.05);
  Buildings.Controls.OBC.CDL.Reals.Sources.Sin measuredFlow(
    amplitude=0.2, offset=0.35, freqHz=1/3600,
    phase=-1.5707963267948966);
  Buildings.Controls.OBC.CDL.Reals.Sources.Constant setFlow(k=0.5);
  Buildings.Controls.OBC.CDL.Logical.Sources.Constant fan(k=true);
  Buildings.Controls.OBC.CDL.Integers.Sources.Constant mode(k=1);
  Buildings.Controls.OBC.CDL.Reals.Sources.Constant damper(k=0.5);
  Buildings.Controls.OBC.CDL.Reals.Sources.Constant valve(k=0.5);
  Buildings.Controls.OBC.CDL.Reals.Sources.Constant supply(k=288.15);
  Buildings.Controls.OBC.CDL.Logical.Sources.Constant hotPlant(k=true);
  Buildings.Controls.OBC.CDL.Reals.Sources.Constant discharge(k=294.15);
  Buildings.Controls.OBC.CDL.Reals.Sources.Constant dischargeSet(k=286.15);
equation
  connect(measuredFlow.y, active.VDis_flow);
  connect(setFlow.y, active.VActSet_flow);
  connect(fan.y, active.u1Fan);
  connect(mode.y, active.uOpeMod);
  connect(damper.y, active.uDam);
  connect(valve.y, active.uVal);
  connect(supply.y, active.TSup);
  connect(hotPlant.y, active.u1HotPla);
  connect(discharge.y, active.TDis);
  connect(dischargeSet.y, active.TDisSet);
  connect(measuredFlow.y, suppressed.VDis_flow);
  connect(setFlow.y, suppressed.VActSet_flow);
  connect(fan.y, suppressed.u1Fan);
  connect(mode.y, suppressed.uOpeMod);
  connect(damper.y, suppressed.uDam);
  connect(valve.y, suppressed.uVal);
  connect(supply.y, suppressed.TSup);
  connect(hotPlant.y, suppressed.u1HotPla);
  connect(discharge.y, suppressed.TDis);
  connect(dischargeSet.y, suppressed.TDisSet);
end G36LowFlowImportance;
