within;
model G36LeakingDamperPositive
  "Drive the pinned OBC reheat alarm through an active leaking-damper episode"
  Buildings.Controls.OBC.ASHRAE.G36.TerminalUnits.Reheat.Subsequences.Alarms ala(
    staPreMul=1, hotWatRes=1, VCooMax_flow=0.95, floHys=0.05,
    damPosHys=0.05, valPosHys=0.05);
  Buildings.Controls.OBC.CDL.Reals.Sources.Sin measuredFlow(
    amplitude=0.1, offset=0.1, freqHz=1/3600,
    phase=-1.5707963267948966);
  Buildings.Controls.OBC.CDL.Reals.Sources.Constant setFlow(k=0.5);
  Buildings.Controls.OBC.CDL.Logical.Sources.Constant fan(k=true);
  Buildings.Controls.OBC.CDL.Integers.Sources.Constant mode(k=1);
  Buildings.Controls.OBC.CDL.Reals.Sources.Constant damper(k=0);
  Buildings.Controls.OBC.CDL.Reals.Sources.Constant valve(k=0.5);
  Buildings.Controls.OBC.CDL.Reals.Sources.Constant supply(k=288.15);
  Buildings.Controls.OBC.CDL.Logical.Sources.Constant hotPlant(k=true);
  Buildings.Controls.OBC.CDL.Reals.Sources.Constant discharge(k=294.15);
  Buildings.Controls.OBC.CDL.Reals.Sources.Constant dischargeSet(k=286.15);
equation
  connect(measuredFlow.y, ala.VDis_flow);
  connect(setFlow.y, ala.VActSet_flow);
  connect(fan.y, ala.u1Fan);
  connect(mode.y, ala.uOpeMod);
  connect(damper.y, ala.uDam);
  connect(valve.y, ala.uVal);
  connect(supply.y, ala.TSup);
  connect(hotPlant.y, ala.u1HotPla);
  connect(discharge.y, ala.TDis);
  connect(dischargeSet.y, ala.TDisSet);
end G36LeakingDamperPositive;
