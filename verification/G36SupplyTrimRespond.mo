within;
model G36SupplyTrimRespond
  "Exercise the pinned OBC trim-and-respond loop with AHU supply-air parameters"
  Buildings.Controls.OBC.ASHRAE.G36.Generic.TrimAndRespond reset(
    iniSet=18, minSet=12, maxSet=18, delTim=600,
    samplePeriod=120, numIgnReq=2, triAmo=0.1,
    resAmo=-0.2, maxRes=-0.6);
  Buildings.Controls.OBC.CDL.Reals.Sources.Sin requestWave(
    amplitude=2.5, offset=2.5, freqHz=1/3600, phase=0.1);
  Buildings.Controls.OBC.CDL.Reals.Round requestRound(n=0);
  Buildings.Controls.OBC.CDL.Conversions.RealToInteger requestCount;
  Buildings.Controls.OBC.CDL.Logical.Sources.Pulse fan(period=2400, width=0.75);
equation
  connect(requestWave.y, requestRound.u);
  connect(requestRound.y, requestCount.u);
  connect(requestCount.y, reset.numOfReq);
  connect(fan.y, reset.uDevSta);
end G36SupplyTrimRespond;
