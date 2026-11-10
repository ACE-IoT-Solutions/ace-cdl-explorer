within;
model G36TimeSuppressionRepeated
  "Exercise setpoint changes during and after the G36 suppression window"
  Buildings.Controls.OBC.ASHRAE.G36.Generic.TimeSuppression timSup;
  Modelica.Blocks.Sources.RealExpression setpoint(
    y=if time < 300 then 300.15 else if time < 510 then 297.15
      else if time < 1230 then 296.15 else 301.15);
  Buildings.Controls.OBC.CDL.Reals.Sources.Constant zone(k=298.15);
equation
  connect(setpoint.y, timSup.TSet);
  connect(zone.y, timSup.TZon);
end G36TimeSuppressionRepeated;
