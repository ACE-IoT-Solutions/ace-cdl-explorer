within;
model G36PressureRequestsPositive
  "Drive the pinned OBC Reheat SystemRequests through positive pressure tiers"
  Buildings.Controls.OBC.ASHRAE.G36.TerminalUnits.Reheat.Subsequences.SystemRequests req(
    floHys=0.01, looHys=0.01, damPosHys=0.1, valPosHys=0.01,
    samplePeriod=120);
  Buildings.Controls.OBC.CDL.Reals.Sources.Sin dam(
    amplitude=0.15, offset=0.85, freqHz=1/3600);
  Buildings.Controls.OBC.CDL.Reals.Sources.Constant setFlow(k=1);
  Buildings.Controls.OBC.CDL.Reals.Sources.Sin measuredFlow(
    amplitude=0.35, offset=0.7, freqHz=1/2400);
  Buildings.Controls.OBC.CDL.Reals.Sources.Constant coolSet(k=297.15);
  Buildings.Controls.OBC.CDL.Reals.Sources.Constant zoneTemp(k=297.15);
  Buildings.Controls.OBC.CDL.Reals.Sources.Constant coolLoop(k=0);
  Buildings.Controls.OBC.CDL.Reals.Sources.Constant disSet(k=303.15);
  Buildings.Controls.OBC.CDL.Reals.Sources.Constant disTemp(k=303.15);
  Buildings.Controls.OBC.CDL.Reals.Sources.Constant heatValve(k=0);
  Buildings.Controls.OBC.CDL.Logical.Sources.Constant afterSupp(k=true);
equation
  connect(dam.y, req.uDam);
  connect(setFlow.y, req.VSet_flow);
  connect(measuredFlow.y, req.VDis_flow);
  connect(coolSet.y, req.TCooSet);
  connect(zoneTemp.y, req.TZon);
  connect(coolLoop.y, req.uCoo);
  connect(disSet.y, req.TDisSet);
  connect(disTemp.y, req.TDis);
  connect(heatValve.y, req.uVal);
  connect(afterSupp.y, req.uAftSup);
end G36PressureRequestsPositive;
