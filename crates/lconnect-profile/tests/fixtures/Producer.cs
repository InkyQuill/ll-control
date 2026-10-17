// Owned synthetic DTOs/values, matching the public JSON shape observed in2.1.28.
// No vendor assembly, user configuration, device API or proprietary asset is used.
using System;
using System.Collections.Generic;
using System.IO;
using System.IO.Compression;
using System.Text;
using System.Text.Json;
public enum ProfileTypes { FanAndLighting, Screen }
public enum MainTypes { Controller, QuickSyncLighting, WMergeLightingEffects }
public sealed class BackupData {
 public MainTypes MainType { get; set; }
 public int SubType { get; set; }
 public object Metadata { get; set; }
 public byte[] Data { get; set; }
}
public sealed class BackupCollection {
 public string Version { get; set; }
 public DateTime? CreatedAt { get; set; }
 public ProfileTypes Type { get; set; }
 public List<BackupData> Datas { get; set; }
}
public static class Producer {
 public static void Main(string[] args) {
  foreach (ProfileTypes kind in new[] { ProfileTypes.FanAndLighting, ProfileTypes.Screen }) {
   var profile = JsonSerializer.SerializeToUtf8Bytes(new { Fan = new { Mode = 0 }, Pump = new { Mode = 0 }, ScreenBrightness = 50, ScreenRotation = 0, ScreenConfigJson = "{}" });
   var backup = new BackupCollection { Version = "2.1.28.0", CreatedAt = new DateTime(2026,10,17,0,0,0,DateTimeKind.Utc), Type = kind,
    Datas = new List<BackupData> { new BackupData { MainType = MainTypes.Controller, SubType = 16846849, Metadata = "OWNED-SYNTHETIC-DEVICE-PATH", Data = profile } } };
   using (var file = File.Create(Path.Combine(args[0], kind.ToString() + ".zip")))
   using (var zip = new ZipArchive(file, ZipArchiveMode.Create)) {
    var entry = zip.CreateEntry("backup", CompressionLevel.Optimal);
    entry.LastWriteTime = new DateTimeOffset(2026,10,17,0,0,0,TimeSpan.Zero);
    var bytes = JsonSerializer.SerializeToUtf8Bytes(backup);
    using (var stream = entry.Open()) stream.Write(bytes, 0, bytes.Length);
    entry = zip.CreateEntry("wmsq/own.txt", CompressionLevel.Optimal);
    entry.LastWriteTime = new DateTimeOffset(2026,10,17,0,0,0,TimeSpan.Zero);
    bytes = Encoding.UTF8.GetBytes("Owned fixture; asset is intentionally not a decodable media file.");
    using (var stream = entry.Open()) stream.Write(bytes, 0, bytes.Length);
   }
  }
 }
}
