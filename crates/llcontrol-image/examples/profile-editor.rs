//! Offline editor latency/RSS fixture. Monitor the release executable with
//! wait4 or GNU time; generate the source and compile outside the measured process.
use llcontrol_image::{
    editor::{prepare_edited, Transform, MAX_TRANSFORMS},
    editor_filters::ColorFilters,
};
use std::{env, fs, io, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.len() != 4 {
        return Err(io::Error::other(
            "usage: profile-editor SOURCE OUTPUT TRANSFORM_COUNT neutral|point|all",
        )
        .into());
    }
    let count: usize = args[2].parse()?;
    if count > MAX_TRANSFORMS {
        return Err(io::Error::other("transform count exceeds editor limit").into());
    }
    let filters = match args[3].as_str() {
        "neutral" => ColorFilters::default(),
        "point" => ColorFilters {
            brightness: 5.0,
            ..Default::default()
        },
        "all" => ColorFilters {
            hue: 27.0,
            saturation: 12.0,
            brightness: 5.0,
            contrast: 10.0,
            sharpen: 5.0,
            blur: 2.5,
        },
        _ => return Err(io::Error::other("unknown filter preset").into()),
    };
    let actions = [
        Transform::RotateRight,
        Transform::FlipHorizontal,
        Transform::RotateLeft,
        Transform::FlipVertical,
    ];
    let transforms: Vec<_> = (0..count).map(|i| actions[i % actions.len()]).collect();
    let source = fs::read(&args[0])?;
    let start = Instant::now();
    let output = prepare_edited(&source, None, &transforms, filters)?;
    let elapsed = start.elapsed();
    let frame = image::load_from_memory(&output)?;
    assert_eq!((frame.width(), frame.height()), (480, 480));
    fs::write(&args[1], &output)?;
    println!(
        "elapsed_ms={} source_bytes={} output_bytes={} transforms={} filters={}",
        elapsed.as_millis(),
        source.len(),
        output.len(),
        count,
        args[3]
    );
    Ok(())
}
