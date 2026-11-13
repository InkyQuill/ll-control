use llcontrol_daemon::sensor_selection::{candidates, validate_selection};
use llcontrol_ipc::*;
fn row(id: &str, unit: SensorUnit) -> SensorReading {
    SensorReading {
        id: id.into(),
        label: "GPU A".into(),
        unit,
        value: None,
        status: SensorStatus::Unavailable,
        observed_unix_ms: None,
        age_ms: None,
    }
}
fn selected(gpu: &str, load: &str) -> SensorDefaults {
    SensorDefaults {
        gpu: Some(GpuSensorDefaults {
            device_id: gpu.into(),
            load: Some(load.into()),
            temperature: None,
            clock: None,
            power: None,
            fan: None,
            voltage: None,
            memory_used: None,
        }),
        ..Default::default()
    }
}
#[test]
fn physical_gpu_membership_prevents_cross_device_selection_and_silent_remap() {
    let readings = vec![
        row("amdgpu:2f61:utilization", SensorUnit::Percent),
        row("amdgpu:2f62:utilization", SensorUnit::Percent),
        row("hwmon:2f62:amdgpu:temp1", SensorUnit::Celsius),
    ];
    let initial = SensorDefaults::default();
    let valid = selected("sysfs:2f62:amdgpu", "amdgpu:2f62:utilization");
    validate_selection(&valid, &initial, &readings).unwrap();
    let wrong = selected("sysfs:2f62:amdgpu", "amdgpu:2f61:utilization");
    assert!(validate_selection(&wrong, &initial, &readings).is_err());
    validate_selection(&valid, &valid, &[]).unwrap();
    assert!(validate_selection(&valid, &initial, &[]).is_err());
    let mut changed_gpu = valid.clone();
    changed_gpu.gpu.as_mut().unwrap().device_id = "sysfs:2f61:amdgpu".into();
    assert!(validate_selection(&changed_gpu, &valid, &readings).is_err());
    validate_selection(&initial, &valid, &[]).unwrap();
    assert!(
        candidates(&readings)
            .iter()
            .filter(|c| c.gpu_id.as_deref() == Some("sysfs:2f62:amdgpu"))
            .count()
            == 2
    );
}
#[test]
fn known_provider_roles_exclude_rails_memory_clocks_malformed_engines_and_wrong_units() {
    let rows = vec![
        row("proc:cpu:utilization", SensorUnit::Percent),
        row("hwmon:2f61:coretemp:temp1", SensorUnit::Celsius),
        row("cpufreq:cpus-0.1:cpuinfo_cur_freq", SensorUnit::Hertz),
        row("powercap:rapl:package-0:power", SensorUnit::Watts),
        row("hwmon:2f62:amdgpu:in0", SensorUnit::Volts),
        row("hwmon:2f62:amdgpu:in1", SensorUnit::Volts),
        row("hwmon:2f62:amdgpu:freq2", SensorUnit::Hertz),
        row(
            "intel-clocks:2f63:xe:tile0:gt1:actual_frequency",
            SensorUnit::Hertz,
        ),
        row(
            "nvidia:GPU-12345678-1234-1234-1234-123456789abc:graphics_clock",
            SensorUnit::Hertz,
        ),
        row(
            "nvidia:GPU-12345678-1234-1234-1234-123456789abc:memory_clock",
            SensorUnit::Hertz,
        ),
        row("hwmon:2f64:spd5118:temp1", SensorUnit::Celsius),
        row("amdgpu:2f65:vram_utilization", SensorUnit::Percent),
        row("amdgpu:2f65:utilization", SensorUnit::Celsius),
        row("intel:2f65:engine:load", SensorUnit::Percent),
    ];
    let choices = candidates(&rows);
    assert_eq!(choices.len(), 7);
    assert_eq!(choices[0].role, DefaultSensorRole::CpuLoad);
    assert!(choices[..4].iter().all(|c| c.gpu_id.is_none()));
    assert_eq!(choices[4].role, DefaultSensorRole::GpuVoltage);
    assert_eq!(choices[5].gpu_id.as_deref(), Some("sysfs:2f63:xe"));
}
#[test]
fn ambiguous_unknown_and_cpu_gpu_role_confusion_are_rejected() {
    let temperature = row("hwmon:2f61:coretemp:temp1", SensorUnit::Celsius);
    assert!(candidates(&[temperature.clone(), temperature]).is_empty());
    let rows = [row("hwmon:2f62:amdgpu:temp1", SensorUnit::Celsius)];
    let mut proposed = SensorDefaults::default();
    proposed.cpu.temperature = Some(rows[0].id.clone());
    assert!(validate_selection(&proposed, &SensorDefaults::default(), &rows).is_err());
    assert!(candidates(&[
        row("hwmon:bad:coretemp:temp1", SensorUnit::Celsius),
        row("nvidia:GPU-bad:temperature", SensorUnit::Celsius)
    ])
    .is_empty());
}

#[test]
fn actual_hwmon_provider_output_groups_related_gpu_metrics_without_labels() {
    use std::{fs, os::unix::fs::symlink};
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("proc")).unwrap();
    fs::create_dir(dir.path().join("hwmon")).unwrap();
    for (index, driver) in [(0, "coretemp"), (1, "amdgpu"), (2, "amdgpu")] {
        let chip = dir.path().join(format!("hwmon/hwmon{index}"));
        let physical = dir.path().join(format!("device-{index}"));
        fs::create_dir(&chip).unwrap();
        fs::create_dir(&physical).unwrap();
        symlink(&physical, chip.join("device")).unwrap();
        fs::write(chip.join("name"), driver).unwrap();
        fs::write(chip.join("temp1_input"), "42000").unwrap();
        fs::write(chip.join("temp1_label"), "identical label").unwrap();
        if driver == "amdgpu" {
            fs::write(physical.join("gpu_busy_percent"), "20").unwrap();
        }
    }
    let mut provider = llcontrol_daemon::sensors::LinuxSensors::new(
        dir.path().join("hwmon"),
        dir.path().join("proc"),
    );
    let readings = provider.collect();
    let choices = candidates(&readings);
    let gpus: std::collections::BTreeSet<_> =
        choices.iter().filter_map(|c| c.gpu_id.clone()).collect();
    assert_eq!(gpus.len(), 2);
    for gpu in gpus {
        let group: Vec<_> = choices
            .iter()
            .filter(|c| c.gpu_id.as_ref() == Some(&gpu))
            .collect();
        assert!(group
            .iter()
            .any(|c| c.role == DefaultSensorRole::GpuTemperature));
        let load = group
            .iter()
            .find(|c| c.role == DefaultSensorRole::GpuLoad)
            .unwrap();
        validate_selection(
            &selected(&gpu, &load.reading.id),
            &SensorDefaults::default(),
            &readings,
        )
        .unwrap();
    }
    let cpu = choices
        .iter()
        .find(|c| c.role == DefaultSensorRole::CpuTemperature)
        .unwrap();
    assert!(cpu.gpu_id.is_none());
    let mut defaults = SensorDefaults::default();
    defaults.cpu.temperature = Some(cpu.reading.id.clone());
    // Even unchanged choices must reject newly contradictory catalog metadata.
    let wrong = row(&cpu.reading.id, SensorUnit::Percent);
    assert!(validate_selection(&defaults, &defaults, &[wrong]).is_err());
}

#[test]
fn recommendation_is_order_independent_fresh_and_never_mixes_gpus() {
    use llcontrol_daemon::sensor_selection::recommended;
    let mut rows = vec![
        row("cpufreq:cpus-0.1:scaling_cur_freq", SensorUnit::Hertz),
        row("cpufreq:cpus-0.1:cpuinfo_cur_freq", SensorUnit::Hertz),
        row("amdgpu:2f62:utilization", SensorUnit::Percent),
        row("amdgpu:2f61:utilization", SensorUnit::Percent),
        row("hwmon:2f62:amdgpu:temp1", SensorUnit::Celsius),
    ];
    for reading in &mut rows {
        reading.status = SensorStatus::Valid;
        reading.value = Some(1.0);
        reading.age_ms = Some(0);
    }
    let first = recommended(&rows, 42);
    assert_eq!(first.revision, 42);
    assert_eq!(
        first.cpu.clock.as_deref(),
        Some("cpufreq:cpus-0.1:cpuinfo_cur_freq")
    );
    assert_eq!(first.gpu.as_ref().unwrap().device_id, "sysfs:2f61:amdgpu");
    assert!(first.gpu.as_ref().unwrap().temperature.is_none());
    validate_selection(&first, &SensorDefaults::default(), &rows).unwrap();
    rows.reverse();
    assert_eq!(recommended(&rows, 42), first);
    for reading in &mut rows {
        reading.age_ms = Some(3001);
    }
    assert_eq!(
        recommended(&rows, 42),
        SensorDefaults {
            revision: 42,
            ..Default::default()
        }
    );
    // A proposal does not modify the caller's stored config or input catalog.
    assert_eq!(
        first.gpu.unwrap().load.as_deref(),
        Some("amdgpu:2f61:utilization")
    );
}

#[test]
fn generic_hwmon_fan_requires_explicit_cpu_assignment_and_never_recommends_wiring() {
    use llcontrol_daemon::sensor_selection::recommended;
    let id = "hwmon:2f61:nct6775:fan1";
    let mut fan = row(id, SensorUnit::Rpm);
    fan.label = "CPU Fan".into();
    fan.status = SensorStatus::Valid;
    fan.value = Some(0.0);
    fan.age_ms = Some(0);
    let readings = vec![fan];
    let choices = candidates(&readings);
    assert_eq!(choices.len(), 1);
    assert_eq!(choices[0].role, DefaultSensorRole::CpuFan);
    assert!(recommended(&readings, 0).cpu.fan.is_none());
    let mut selected = SensorDefaults::default();
    selected.cpu.fan = Some(id.into());
    validate_selection(&selected, &SensorDefaults::default(), &readings).unwrap();
    validate_selection(&selected, &selected, &[]).unwrap();
    assert!(validate_selection(&selected, &SensorDefaults::default(), &[]).is_err());
    for bad in [
        "hwmon:2f61:amdgpu:fan1",
        "hwmon:2f61:nouveau:fan1",
        "hwmon:2f61:xe:fan1",
        "hwmon:2f61:i915:fan1",
        "hwmon:2f61:nct6775:fan0",
        "hwmon:2f61:nct6775:fan+1",
        "hwmon:2f61:bad driver:fan1",
        "lcdv2:fixture:fan_rpm",
    ] {
        let rows = [row(bad, SensorUnit::Rpm)];
        assert!(
            !candidates(&rows)
                .iter()
                .any(|c| c.role == DefaultSensorRole::CpuFan),
            "{bad}"
        );
    }
    assert!(candidates(&[row(id, SensorUnit::Celsius)]).is_empty());
}

#[test]
fn intel_engine_is_an_explicit_source_not_an_automatic_whole_gpu_estimate() {
    for id in [
        "intel:2f65:i915:engine:0:utilization",
        "intel:2f65:xe:gt0:class0:instance0:utilization",
    ] {
        let mut reading = row(id, SensorUnit::Percent);
        reading.value = Some(25.0);
        reading.status = SensorStatus::Valid;
        reading.age_ms = Some(0);
        reading.observed_unix_ms = Some(1);
        let rows = vec![reading];
        let choices = llcontrol_daemon::sensor_selection::candidates(&rows);
        assert_eq!(choices.len(), 1);
        assert_eq!(choices[0].role, llcontrol_ipc::DefaultSensorRole::GpuLoad);
        let mut defaults = llcontrol_daemon::sensor_selection::recommended(&rows, 0);
        assert!(defaults.gpu.as_ref().unwrap().load.is_none());
        defaults.gpu.as_mut().unwrap().load = Some(id.into());
        llcontrol_daemon::sensor_selection::validate_selection(
            &defaults,
            &Default::default(),
            &rows,
        )
        .unwrap();
    }
    for id in [
        "intel:2f65:i915:engine:1:utilization",
        "intel:2f65:i915:engine:100000:utilization",
        "intel:2f65:xe:gt16:class0:instance0:utilization",
        "intel:2f65:xe:gt0:class5:instance0:utilization",
        "intel:2f65:xe:gt0:class0:instance256:utilization",
    ] {
        assert!(
            llcontrol_daemon::sensor_selection::candidates(&[row(id, SensorUnit::Percent)])
                .is_empty()
        );
    }
}
