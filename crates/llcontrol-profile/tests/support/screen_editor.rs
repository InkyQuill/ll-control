//! Owned, complete basic editor DTO values; no vendor configuration/assets.
use serde_json::{json, Value};
pub fn editor(image: bool, slot: usize) -> Value {
    let aligned = |colors: Value| json!({"LabelColor":colors,"FontFamily":5,"VerticalAlignment":1,"HorizontalAlignment":2,"TextAlignment":0,"Shadow":29});
    let mut slider = aligned(json!([[11, 22, 33], [44, 55, 66], [77, 88, 99]]));
    slider["SensorTypes"] = json!([6, 1, 4]);
    slider["LoopSecond"] = json!(13);
    let mut custom = aligned(json!([[12, 23, 34]]));
    custom["Text"] = json!(format!("Owned editor slot{slot}"));
    custom["FontSize"] = json!(2);
    let grid = |ids: Value| json!({"LabelColor":[[17,18,19],[20,21,22]],"FontFamily":5,"Shadow":37,"SensorTypes":ids});
    let mut value = json!({"TextThemeType":6,"TextSliderSensor":slider,"TextDuoSensor":{"SensorTypes":[5,2],"LabelColor":[[31,32,33],[34,35,36],[37,38,39]],"FontFamily":5,"VerticalAlignment":2,"Shadow":41},"TextThreeSensor":grid(json!([6,1,3])),"TextFourSensor":grid(json!([5,2,3,4])),"TextTime":aligned(json!([[41,42,43],[44,45,46],[47,48,49]])),"TextCustom":custom,"Watermark":{"HasImage":false,"Image":"","Horizontal":50,"Vertical":50,"Shadow":50,"Opacity":50}});
    if image {
        value["ImageVideoPath"] = json!("");
        value["BackgroundColor"] = json!([slot, 10, 20]);
    } else {
        value["BackgroundColor"] = json!({"Type":2,"ColorSettings":{"SolidColor":{"Colors":[[slot,1,2]],"Flip":false},"HorizontalGradient":{"Colors":[[3,4,5],[6,7,8]],"Flip":true},"VerticalGradient":{"Colors":[[9,10,11],[12,13,14]],"Flip":false},"RadialGradient":{"Colors":[[15,16,17],[18,19,20]],"Flip":true}}});
    }
    value
}
