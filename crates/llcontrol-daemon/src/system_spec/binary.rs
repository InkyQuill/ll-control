use super::*;
fn u16_at(data: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        data.get(offset..offset + 2)?.try_into().ok()?,
    ))
}
fn u32_at(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        data.get(offset..offset + 4)?.try_into().ok()?,
    ))
}
pub(super) fn dimm(raw: &[u8], id: &str) -> Option<Item> {
    let length = *raw.get(1)? as usize;
    if raw.first() != Some(&17) || length < 21 || length > raw.len() {
        return None;
    }
    let data = &raw[..length];
    let size = u16_at(data, 12)?;
    if size == 0 {
        return None;
    }
    let tail = &raw[length..];
    let end = tail.windows(2).position(|s| s == [0, 0])?;
    let strings = tail[..end].split(|b| *b == 0).collect::<Vec<_>>();
    let string = |offset: usize| {
        data.get(offset)
            .and_then(|index| index.checked_sub(1))
            .and_then(|i| strings.get(i as usize))
            .and_then(|v| std::str::from_utf8(v).ok())
            .and_then(clean)
    };
    let bytes = match size {
        0xffff => None,
        0x7fff => u32_at(data, 28).map(|v| u64::from(v & 0x7fffffff) * 1024 * 1024),
        n => Some(u64::from(n & 0x7fff) * if n & 0x8000 != 0 { 1024 } else { 1024 * 1024 }),
    }
    .filter(|n| *n > 0);
    let memory_type = match data[18] {
        0x18 => Some("DDR3".into()),
        0x1a => Some("DDR4".into()),
        0x22 => Some("DDR5".into()),
        0x1b => Some("LPDDR".into()),
        0x1c => Some("LPDDR2".into()),
        0x1d => Some("LPDDR3".into()),
        0x1e => Some("LPDDR4".into()),
        0x23 => Some("LPDDR5".into()),
        0..=2 => None,
        n => Some(format!("SMBIOS type 0x{n:02x}")),
    };
    let speed = |offset, extended| {
        u16_at(data, offset)
            .and_then(|v| match v {
                0 => None,
                0xffff if data.len() >= 0x5c => u32_at(data, extended).filter(|n| *n > 0),
                0xffff => None,
                v => Some(u32::from(v)),
            })
            .map(|v| format!("{v} MT/s"))
    };
    Some(Item {
        id: format!("dimm:{id}"),
        title: string(16)
            .or_else(|| string(26))
            .unwrap_or_else(|| format!("Memory device {id}")),
        fields: vec![
            txt("manufacturer", string(23), "SMBIOS type17"),
            txt("part_number", string(26), "SMBIOS type17"),
            txt("locator", string(16), "SMBIOS type17"),
            txt("bank", string(17), "SMBIOS type17"),
            txt("memory_type", memory_type, "SMBIOS type17"),
            field("capacity", bytes.map(Value::Bytes), "SMBIOS type17"),
            txt("rated_transfer_rate", speed(21, 84), "SMBIOS type17"),
            txt("configured_transfer_rate", speed(32, 88), "SMBIOS type17"),
        ],
    })
}
pub(super) fn edid(raw: &[u8]) -> Option<(Option<String>, Vec<Field>)> {
    let data = raw.get(..128)?;
    if data[..8] != [0, 255, 255, 255, 255, 255, 255, 0]
        || data.iter().fold(0u8, |sum, v| sum.wrapping_add(*v)) != 0
        || data[18] != 1
    {
        return None;
    }
    let code = u16::from_be_bytes([data[8], data[9]]);
    let mut manufacturer = String::new();
    for shift in [10, 5, 0] {
        let c = ((code >> shift) & 31) as u8;
        if !(1..=26).contains(&c) {
            return None;
        }
        manufacturer.push((b'A' + c - 1) as char);
    }
    let mut name = None;
    for descriptor in data[54..126].chunks_exact(18) {
        if descriptor[..3] == [0, 0, 0] && descriptor[3] == 0xfc {
            name = std::str::from_utf8(&descriptor[5..18]).ok().and_then(clean);
        }
    }
    let timing = if u16_at(data, 54)? != 0 {
        let h = u16::from(data[56]) | (u16::from(data[58] & 0xf0) << 4);
        let mut v = u16::from(data[59]) | (u16::from(data[61] & 0xf0) << 4);
        let interlaced = data[71] & 0x80 != 0;
        if interlaced {
            v *= 2;
        }
        Some(format!(
            "{h} × {v}{}",
            if interlaced { " (interlaced)" } else { "" }
        ))
        .filter(|_| h > 0 && v > 0)
    } else {
        None
    };
    let size = (data[21] > 0 && data[22] > 0).then(|| format!("{} × {} cm", data[21], data[22]));
    Some((
        name.clone(),
        vec![
            txt("model", name, "EDID base block"),
            txt("manufacturer", Some(manufacturer), "EDID PNP ID"),
            txt(
                "product_id",
                Some(format!("0x{:04x}", u16_at(data, 10)?)),
                "EDID base block",
            ),
            txt("physical_size", size, "EDID base block"),
            txt("detailed_timing", timing, "EDID base block"),
        ],
    ))
}
