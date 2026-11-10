within ;
model ACEOptimalStartNoRecovery
  "Deterministic event inputs for the pinned OBC optimal-start calculation"
  constant Real day(unit="s") = 86400;
  Real timeOfDay(unit="s");
  Real temperatureDifference(unit="K");
  Boolean startCalculation;
  Real secondsUntilOccupied(unit="s");
  Buildings.Controls.OBC.Utilities.BaseClasses.OptimalStartCalculation calculation(
    tOptMax=10800,
    thrOptOn=60,
    nDay=3,
    uLow=0,
    uHigh=0.5);
equation
  timeOfDay = mod(time, day);
  // A partial recovery starts at 05:00 and stays 0.05 K short. The calculation
  // samples at the start of that window; occupancy begins at 08:00.
  temperatureDifference = if timeOfDay < 18000 then 3 else
    if timeOfDay < 21600 then 3 - 2.95*(timeOfDay - 18000)/3600 else 0.05;
  startCalculation = timeOfDay >= 18000 and timeOfDay < 18060;
  secondsUntilOccupied = mod(28800 - time, day);
  calculation.TDif = temperatureDifference;
  calculation.staCal = startCalculation;
  calculation.tNexOcc = secondsUntilOccupied;
  annotation (experiment(StartTime=0, StopTime=259200, Interval=60, Tolerance=1e-8));
end ACEOptimalStartNoRecovery;
