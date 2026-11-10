model G36ConnectedGroupModeTransitions
  "Exercise zone-group mode transitions through both pinned OBC blocks"
  Buildings.Controls.OBC.ASHRAE.G36.ZoneGroups.GroupStatus zonGroSta(
    nBuiZon=2, nGroZon=2);
  Buildings.Controls.OBC.ASHRAE.G36.ZoneGroups.OperationMode opeModSel(
    nZon=2);
  Integer phase;
  Real zoneTemperature(unit="K");

equation
  phase = if time < 360 then 0 else if time < 720 then 1 else
    if time < 1080 then 2 else if time < 1440 then 3 else
    if time < 1800 then 4 else 5;
  zoneTemperature = if phase == 2 then 280.15 else
    if phase == 3 then 310.15 else if phase == 4 then 270.15 else 295.15;

  zonGroSta.zonOcc = fill(false, 2);
  zonGroSta.u1Occ = fill(phase == 0 or phase == 5, 2);
  zonGroSta.tNexOcc = fill(10000, 2);
  zonGroSta.uCooTim = fill(600, 2);
  zonGroSta.uWarTim = fill(600, 2);
  zonGroSta.u1OccHeaHig = fill(false, 2);
  zonGroSta.u1HigOccCoo = fill(false, 2);
  zonGroSta.u1UnoHeaHig = fill(phase == 2, 2);
  zonGroSta.THeaSetOff = fill(if phase == 4 then 260.15 else 285.15, 2);
  zonGroSta.u1EndSetBac = fill(phase <> 2, 2);
  zonGroSta.u1HigUnoCoo = fill(phase == 3, 2);
  zonGroSta.TCooSetOff = fill(303.15, 2);
  zonGroSta.u1EndSetUp = fill(phase <> 3, 2);
  zonGroSta.TZon = fill(zoneTemperature, 2);
  zonGroSta.u1Win = fill(true, 2);

  connect(zonGroSta.uGroOcc, opeModSel.u1Occ);
  connect(zonGroSta.nexOcc, opeModSel.tNexOcc);
  connect(zonGroSta.yCooTim, opeModSel.maxCooDowTim);
  connect(zonGroSta.yHigOccCoo, opeModSel.u1HigOccCoo);
  connect(zonGroSta.yWarTim, opeModSel.maxWarUpTim);
  connect(zonGroSta.yOccHeaHig, opeModSel.u1OccHeaHig);
  connect(zonGroSta.yOpeWin, opeModSel.uOpeWin);
  connect(zonGroSta.yColZon, opeModSel.totColZon);
  connect(zonGroSta.ySetBac, opeModSel.u1SetBac);
  connect(zonGroSta.yEndSetBac, opeModSel.u1EndSetBac);
  connect(zonGroSta.TZonMin, opeModSel.TZonMin);
  connect(zonGroSta.yHotZon, opeModSel.totHotZon);
  connect(zonGroSta.ySetUp, opeModSel.u1SetUp);
  connect(zonGroSta.yEndSetUp, opeModSel.u1EndSetUp);
end G36ConnectedGroupModeTransitions;
