// Owned synthetic DTOs/values. No vendor source or assembly is required.
// Compile this file separately from Producer.cs (each is a console program).
using System;
using System.Collections.Generic;
using System.IO;
using System.IO.Compression;
using System.Text.Json;
public enum RpmMode {
 Quiet, StandardSpeed, HighSpeed, FullSpeed, UserDefine1, UserDefine2,
 UserDefine3, UserDefine4, Turbo, FixRPM, PWM, PWMQuiet, PWMPerformance, PWMTurbo
}
public static class CoolingProducer {
 static object Setting(bool pump) {
  var profiles = new Dictionary<RpmMode, object>();
  var modes = pump ? new[] { RpmMode.FixRPM, RpmMode.PWMQuiet, RpmMode.PWMPerformance }
   : new[] { RpmMode.Quiet, RpmMode.StandardSpeed, RpmMode.HighSpeed, RpmMode.FullSpeed,
      RpmMode.UserDefine1, RpmMode.UserDefine2, RpmMode.UserDefine3, RpmMode.UserDefine4 };
  foreach (var mode in modes) {
   bool fixedMode = mode == RpmMode.FixRPM;
   int[] temperatures = pump ? (fixedMode ? new[] { 0 } : new[] { 30, 40, 50, 60, 70, 80 }) : new[] { 25, 40, 55, 70, 90 };
   int[] speeds = pump ? (fixedMode ? new[] { 2300 } : new[] { 2000, 2200, 2400, 2600, 2800, 3000 }) : new[] { 20, 40 + (int)mode, 60, 80, 100 };
   var phases = new List<object>();
   for(int i=0;i<temperatures.Length;i++)phases.Add(new { Temperature=temperatures[i], Speed=speeds[i] });
   var info = new { Adjustable=true, Phases=phases, MinSpeed=pump ? (fixedMode ? 800 : 2000) : 10,
    MaxSpeed=pump ? (fixedMode ? 3600 : 3000) : 100, ChartMinSpeed=0, ChartMaxSpeed=pump?3600:100,
    ChartSpeedTick=pump?200:10, RPMChartXAxisConfigs=(object)null, ChartSpeedLabels=new[] { "Owned low", "Owned high" } };
   profiles.Add(mode, new { Mode=mode, Name=mode==RpmMode.Quiet ? null : "Owned " + mode,
    IsStartStop=false, FanSize=0, RPMReferenceSource=0, ControlType=fixedMode?0:1,
    PhaseInfos=new Dictionary<string,object> { { fixedMode ? "None|CPU|Fixed" : "None|CPU|Curve", info } } });
  }
  return new { SupportedMotherboardSync=pump, SupportedStartStop=false, SupportedRenameProfileName=true,
   SupportedFanSize=new[] { 0 }, SupportedRPMReferenceSource=new[] { 0 }, SpeedUnit=pump?0:1,
   Mode=pump?RpmMode.PWMQuiet:RpmMode.UserDefine2, Profiles=profiles };
 }
 public static void Main(string[] args) {
  var profile = JsonSerializer.SerializeToUtf8Bytes(new { Fan=Setting(false), Pump=Setting(true), FanLEDMode=0, ScreenBrightness=50 });
  var data = new[] {
   new { MainType=0, SubType=16846848, Metadata="OWNED-FIRST-GENERATION", Data=JsonSerializer.SerializeToUtf8Bytes(new {}) },
   new { MainType=0, SubType=16846849, Metadata="OWNED-SYNTHETIC-DEVICE-PATH", Data=profile }
  };
  var backup = new { Version="2.1.28.0", CreatedAt=new DateTime(2026,10,17,0,0,0,DateTimeKind.Utc), Type=0, Datas=data };
  using(var file=File.Create(Path.Combine(args[0],"Cooling.zip")))
  using(var zip=new ZipArchive(file,ZipArchiveMode.Create)) {
   var entry=zip.CreateEntry("backup",CompressionLevel.Optimal);
   entry.LastWriteTime=new DateTimeOffset(2026,10,17,0,0,0,TimeSpan.Zero);
   var bytes=JsonSerializer.SerializeToUtf8Bytes(backup);
   using(var stream=entry.Open())stream.Write(bytes,0,bytes.Length);
  }
 }
}
