//! Owned basic-runtime inputs; not a Windows export or CSS pixel reference.
use serde_json::{json, Value};
pub fn layout() -> Value {
    json!({"FontColors":[[210,20,30],[20,210,30],[20,30,210]],"FontSize":43,"FontFamily":"HyperspaceRace_BoldItalic","HorizontalAlignment":"Center","VerticalAlignment":"Center","TextAlignment":"Center","Shadow":35,"Opacity":80})
}
pub fn source(text: Value) -> Value {
    json!({"Style":"Full","Screens":[{"Theme":{"Type":"Image","Path":"","BackgroundColor":[3,5,7]},"Text":text,"Watermark":{"HasImage":false,"Image":"","Horizontal":50,"Vertical":50,"Shadow":50,"Opacity":50}}]})
}
#[path = "screen_bindings.rs"]
mod binding_fixture;
pub use binding_fixture::bindings;
