//! Safe decoding of bounded native-endian kernel UAPI byte buffers.
use crate::{Driver, Memory, QueryError};
use std::collections::BTreeSet;
pub(crate) const MAX_BYTES: usize = 16 + 32 * 88;
pub(crate) fn parse(driver: Driver, data: &[u8]) -> Result<Memory, QueryError> {
    let header = if driver == Driver::I915 { 16 } else { 8 };
    if data.len() < header || data.len() > MAX_BYTES {
        return Err(QueryError::Invalid);
    }
    let count = u32::from_ne_bytes(data[..4].try_into().map_err(|_| QueryError::Invalid)?) as usize;
    if count > 32 || data.len() != header + count * 88 || data[4..header].iter().any(|b| *b != 0) {
        return Err(QueryError::Invalid);
    }
    let mut seen = BTreeSet::new();
    let mut total = 0u64;
    let mut used = 0u64;
    for region in data[header..].chunks_exact(88) {
        let class = u16::from_ne_bytes([region[0], region[1]]);
        let instance = u16::from_ne_bytes([region[2], region[3]]);
        if !seen.insert((class, instance)) || class > 1 {
            return Err(QueryError::Invalid);
        }
        // Unknown future classes fail closed; system memory is not VRAM.
        if class == 0 {
            continue;
        }
        let size = u64::from_ne_bytes(region[8..16].try_into().map_err(|_| QueryError::Invalid)?);
        let counter =
            u64::from_ne_bytes(region[16..24].try_into().map_err(|_| QueryError::Invalid)?);
        if size == 0 || counter > size {
            return Err(QueryError::Invalid);
        }
        let allocated = if driver == Driver::I915 {
            size - counter
        } else {
            counter
        };
        total = total.checked_add(size).ok_or(QueryError::Invalid)?;
        used = used.checked_add(allocated).ok_or(QueryError::Invalid)?;
    }
    if total == 0 {
        return Err(QueryError::Unavailable);
    }
    // Both drivers redact accounting to zero allocated bytes without privilege.
    // Nonzero proves accounting in THIS response; never retain that conclusion
    // across polls. Even privileged all-zero results remain ambiguous here.
    Ok(Memory {
        total,
        used: (used > 0).then_some(used),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn payload(driver: Driver, regions: &[(u16, u16, u64, u64)]) -> Vec<u8> {
        let header = if driver == Driver::I915 { 16 } else { 8 };
        let mut data = vec![0; header + regions.len() * 88];
        data[..4].copy_from_slice(&(regions.len() as u32).to_ne_bytes());
        for (row, (class, instance, total, used)) in
            data[header..].chunks_exact_mut(88).zip(regions)
        {
            row[..2].copy_from_slice(&class.to_ne_bytes());
            row[2..4].copy_from_slice(&instance.to_ne_bytes());
            row[8..16].copy_from_slice(&total.to_ne_bytes());
            row[16..24].copy_from_slice(
                &(if driver == Driver::I915 {
                    total - used
                } else {
                    *used
                })
                .to_ne_bytes(),
            );
        }
        data
    }
    #[test]
    fn dedicated_regions_sum_without_sysmem_and_redacted_zero_is_unknown() {
        for driver in [Driver::I915, Driver::Xe] {
            let data = payload(
                driver,
                &[(0, 0, 100000, 10000), (1, 1, 4096, 1024), (1, 2, 8192, 0)],
            );
            assert_eq!(
                parse(driver, &data),
                Ok(Memory {
                    total: 12288,
                    used: Some(1024)
                })
            );
            assert_eq!(
                parse(driver, &payload(driver, &[(1, 1, 4096, 0)])),
                Ok(Memory {
                    total: 4096,
                    used: None
                })
            );
            assert_eq!(
                parse(driver, &payload(driver, &[(0, 0, 100000, 10000)])),
                Err(QueryError::Unavailable)
            );
        }
    }
    #[test]
    fn truncated_duplicate_overflow_unknown_and_inconsistent_responses_rejected() {
        for driver in [Driver::I915, Driver::Xe] {
            let data = payload(driver, &[(1, 1, 4096, 1024)]);
            for size in 0..data.len() {
                assert_eq!(parse(driver, &data[..size]), Err(QueryError::Invalid));
            }
            for entries in [
                vec![(1, 1, 4096, 0), (1, 1, 4096, 0)],
                vec![(1, 1, u64::MAX, 1), (1, 2, 1, 1)],
                vec![(2, 1, 1, 1)],
                vec![(1, 1, 0, 0)],
            ] {
                assert_eq!(
                    parse(driver, &payload(driver, &entries)),
                    Err(QueryError::Invalid)
                );
            }
            let mut bad = data.clone();
            let offset = if driver == Driver::I915 { 16 } else { 8 };
            bad[offset + 16..offset + 24].copy_from_slice(&5000u64.to_ne_bytes());
            assert_eq!(parse(driver, &bad), Err(QueryError::Invalid));
            let mut bad = data.clone();
            bad[4] = 1;
            assert_eq!(parse(driver, &bad), Err(QueryError::Invalid));
            let mut bad = data;
            bad[..4].copy_from_slice(&u32::MAX.to_ne_bytes());
            assert_eq!(parse(driver, &bad), Err(QueryError::Invalid));
        }
    }
}
