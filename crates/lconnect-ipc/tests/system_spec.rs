use lconnect_ipc::*;
fn empty() -> SystemSpec {
    SystemSpec {
        version: 1,
        collected_at_unix_ms: 1,
        groups: [
            SpecKind::Cpu,
            SpecKind::Gpu,
            SpecKind::Memory,
            SpecKind::Monitors,
            SpecKind::Motherboard,
            SpecKind::Storage,
        ]
        .into_iter()
        .map(|kind| SpecGroup {
            kind,
            items: vec![],
            unavailable_reason: Some("No accessible records".into()),
        })
        .collect(),
    }
}
#[test]
fn typed_inventory_preserves_unknowns_and_rejects_ambiguous_or_unbounded_reports() {
    let mut spec = empty();
    spec.validate().unwrap();
    assert_eq!(
        serde_json::from_slice::<SystemSpec>(&serde_json::to_vec(&spec).unwrap()).unwrap(),
        spec
    );
    spec.groups[0].kind = SpecKind::Gpu;
    assert!(spec.validate().is_err());
    spec = empty();
    spec.groups[0].unavailable_reason = None;
    assert!(spec.validate().is_err());
    spec.groups[0].items.push(SpecItem {
        id: "cpu:0".into(),
        title: "CPU".into(),
        fields: vec![SpecField {
            key: "cores".into(),
            value: None,
            source: "fixture".into(),
            unavailable_reason: None,
        }],
    });
    assert!(spec.validate().is_err());
    spec.groups[0].items[0].fields[0].unavailable_reason = Some("Missing".into());
    spec.validate().unwrap();
    spec.groups[0].items[0].fields[0].value = Some(SpecValue::Count(0));
    assert!(spec.validate().is_err());
    let mut oversized = empty();
    oversized.groups[0].unavailable_reason = None;
    oversized.groups[0].items = (0..128)
        .map(|i| SpecItem {
            id: format!("cpu:{i}"),
            title: "CPU".into(),
            fields: (0..32)
                .map(|j| SpecField {
                    key: format!("f{j}"),
                    value: Some(SpecValue::Text("x".repeat(2048))),
                    source: "fixture".into(),
                    unavailable_reason: None,
                })
                .collect(),
        })
        .collect();
    assert!(oversized.validate().is_err());
}
