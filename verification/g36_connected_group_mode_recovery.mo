model G36ConnectedGroupModeRecovery
  "Feed official OBC OperationMode validation signals through GroupStatus"
  extends Buildings.Controls.OBC.ASHRAE.G36.ZoneGroups.Validation.OperationMode;

  Buildings.Controls.OBC.ASHRAE.G36.ZoneGroups.GroupStatus zonGroSta(
    nBuiZon=1, nGroZon=1);
  Buildings.Controls.OBC.ASHRAE.G36.ZoneGroups.OperationMode connectedMode(
    nZon=1);

equation
  zonGroSta.zonOcc[1] = false;
  zonGroSta.u1Win[1] = true;
  connect(occSch.occupied, zonGroSta.u1Occ[1]);
  connect(occSch.tNexOcc, zonGroSta.tNexOcc[1]);
  connect(cooDowTim.y, zonGroSta.uCooTim[1]);
  connect(warUpTim.y, zonGroSta.uWarTim[1]);
  connect(lowThaHeaSet.y, zonGroSta.u1OccHeaHig[1]);
  connect(higThaCooSet.y, zonGroSta.u1HigOccCoo[1]);
  connect(greEqu2.y, zonGroSta.u1UnoHeaHig[1]);
  connect(TZonHeaSetUno.y, zonGroSta.THeaSetOff[1]);
  connect(hys3.y, zonGroSta.u1EndSetBac[1]);
  connect(greEqu3.y, zonGroSta.u1HigUnoCoo[1]);
  connect(TZonCooSetUno.y, zonGroSta.TCooSetOff[1]);
  connect(hys1.y, zonGroSta.u1EndSetUp[1]);
  connect(zonTem.y, zonGroSta.TZon[1]);

  connect(zonGroSta.uGroOcc, connectedMode.u1Occ);
  connect(zonGroSta.nexOcc, connectedMode.tNexOcc);
  connect(zonGroSta.yCooTim, connectedMode.maxCooDowTim);
  connect(zonGroSta.yHigOccCoo, connectedMode.u1HigOccCoo);
  connect(zonGroSta.yWarTim, connectedMode.maxWarUpTim);
  connect(zonGroSta.yOccHeaHig, connectedMode.u1OccHeaHig);
  connect(zonGroSta.yOpeWin, connectedMode.uOpeWin);
  connect(zonGroSta.yColZon, connectedMode.totColZon);
  connect(zonGroSta.ySetBac, connectedMode.u1SetBac);
  connect(zonGroSta.yEndSetBac, connectedMode.u1EndSetBac);
  connect(zonGroSta.TZonMin, connectedMode.TZonMin);
  connect(zonGroSta.yHotZon, connectedMode.totHotZon);
  connect(zonGroSta.ySetUp, connectedMode.u1SetUp);
  connect(zonGroSta.yEndSetUp, connectedMode.u1EndSetUp);
end G36ConnectedGroupModeRecovery;
