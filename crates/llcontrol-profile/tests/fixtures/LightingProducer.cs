// Owned synthetic DTO fixture. No vendor code, assembly, assets or WPF dependency.
using System;
using System.Collections.Generic;
using System.IO;
using System.IO.Compression;
using System.Text.Json;
public enum LightingMode { Unknown, Rainbow, RainbowMorph, StaticColor, Breathing, Runway, Meteor, TickerTape, Fluctuation, Transmit, ColorfulStarryNight, StaticStarryNight, Voice, BigBang, Burst, ColorsMorph, Bounce }
public enum LightingSensor { Unknown, CPULoad, CPUTemperature, GPULoad, GPUTemperature, PumpRPM, CoolantTemp }
public static class LightingProducer {
 static object Parameters(int index, int brightness) {
  int count=new[]{0,0,1,1,2,4,4,4,2,0,1,1,4,1,0,4}[index];
  var colors=new List<int[]>();for(int c=0;c<count;c++)colors.Add(new[]{17+index,c,99});
  return new { Colors=colors, Speed=index==2?int.MinValue:42, Brightness=brightness, Direction=0 };
 }
 public static void Main(string[] args) {
  var fan=new Dictionary<LightingMode,object>();var pump=new Dictionary<LightingMode,object>();
  for(int i=0;i<16;i++) {
   var mode=(LightingMode)(i+1);
   if(i<6) {
    int count=new[]{0,0,1,1,2,4}[i];var colors=new List<int[]>();for(int c=0;c<count;c++)colors.Add(new[]{17+i,c,99});
    fan.Add(mode,new {Mode=mode,Colors=colors,Speed=i==2?int.MinValue:42,Brightness=73,Direction=0});
   }
   var sensors=new Dictionary<LightingSensor,object>();
   for(int s=0;s<6;s++)sensors.Add((LightingSensor)(s+1),new {High=Parameters(i,90-s),Low=Parameters(i,20+s),HighValue=60+s,LowValue=30+s,MinValue=0,MaxValue=3600,UnitText=s==4?"R":s%2==0?"%":"°C"});
   pump.Add(mode,new {Mode=mode,IsDynamicMode=i%2==0,Static=Parameters(i,73),DynamicSettings=sensors,SensorType=(LightingSensor)(i%6+1)});
  }
  var profile=new {FanLEDMode=LightingMode.Breathing,ScreenLEDMode=LightingMode.BigBang,FanLEDProfiles=fan,ScreenLEDProfiles=pump,FanLEDSyncToScreenLED=false,FanLEDSize=0,FanLEDLoopNumber=37,ScreenBrightness=51};
  var data=new[]{new{MainType=0,SubType=16846849,Metadata="PRIVATE-WINDOWS-SERIAL",Data=JsonSerializer.SerializeToUtf8Bytes(profile)}};
  var backup=new{Version="2.1.28.0",Type=0,Datas=data};
  using(var file=File.Create(Path.Combine(args[0],"Lighting.zip")))using(var zip=new ZipArchive(file,ZipArchiveMode.Create)) {
   var entry=zip.CreateEntry("backup",CompressionLevel.Optimal);entry.LastWriteTime=new DateTimeOffset(2026,10,17,0,0,0,TimeSpan.Zero);
   using(var stream=entry.Open()){var bytes=JsonSerializer.SerializeToUtf8Bytes(backup);stream.Write(bytes,0,bytes.Length);}
  }
 }
}
