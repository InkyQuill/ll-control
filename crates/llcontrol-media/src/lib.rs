//! Shared bounded GIF/MP4 preparation and immutable preview owners.
//! Runs on blocking workers; no daemon, GUI, asset publication or HID dependency.
pub mod media_prepare;
pub mod video_adjustments;
pub mod video_range;
