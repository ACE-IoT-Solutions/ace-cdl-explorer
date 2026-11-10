model G36ConnectedGroupModeValidation
  "Connect pinned OBC GroupStatus validation inputs to pinned OperationMode"
  extends Buildings.Controls.OBC.ASHRAE.G36.ZoneGroups.Validation.GroupStatus;

  Buildings.Controls.OBC.ASHRAE.G36.ZoneGroups.OperationMode opeModSel(
    nZon=3) "Group operating mode from aggregate zone status";

equation
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
end G36ConnectedGroupModeValidation;
