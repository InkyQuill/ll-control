//! xe ENGINES query: never infer hardware engine coordinates from card numbers.
use crate::{Engine, QueryError};
use std::collections::BTreeSet;
pub(crate) const MAX_BYTES: usize = 8 + 32 * 32;
pub(crate) fn parse(data: &[u8]) -> Result<Vec<Engine>, QueryError> {
    if data.len() < 8 || data.len() > MAX_BYTES {
        return Err(QueryError::Invalid);
    }
    let count = u32::from_ne_bytes(data[..4].try_into().map_err(|_| QueryError::Invalid)?) as usize;
    if count > 32 || data.len() != 8 + 32 * count || data[4..8].iter().any(|b| *b != 0) {
        return Err(QueryError::Invalid);
    }
    let mut engines = BTreeSet::new();
    for region in data[8..].chunks_exact(32) {
        let class = u16::from_ne_bytes([region[0], region[1]]);
        let instance = u16::from_ne_bytes([region[2], region[3]]);
        let gt = u16::from_ne_bytes([region[4], region[5]]);
        // Only actual hardware classes and coordinates representable by PMU.
        if class > 4
            || instance > 255
            || gt > 15
            || region[6..].iter().any(|b| *b != 0)
            || !engines.insert(Engine {
                gt,
                class,
                instance,
            })
        {
            return Err(QueryError::Invalid);
        }
    }
    Ok(engines.into_iter().collect())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn data(entries: &[(u16, u16, u16)]) -> Vec<u8> {
        let mut data = vec![0; 8 + entries.len() * 32];
        data[..4].copy_from_slice(&(entries.len() as u32).to_ne_bytes());
        for (row, (gt, class, instance)) in data[8..].chunks_exact_mut(32).zip(entries) {
            row[..2].copy_from_slice(&class.to_ne_bytes());
            row[2..4].copy_from_slice(&instance.to_ne_bytes());
            row[4..6].copy_from_slice(&gt.to_ne_bytes());
        }
        data
    }
    #[test]
    fn engine_query_preserves_gt_class_instance_and_rejects_invalid_topology() {
        let bytes = data(&[(1, 4, 2), (0, 0, 0)]);
        assert_eq!(
            parse(&bytes).unwrap(),
            vec![
                Engine {
                    gt: 0,
                    class: 0,
                    instance: 0
                },
                Engine {
                    gt: 1,
                    class: 4,
                    instance: 2
                }
            ]
        );
        for len in 0..bytes.len() {
            assert!(parse(&bytes[..len]).is_err());
        }
        for entries in [
            vec![(0, 0, 0), (0, 0, 0)],
            vec![(16, 0, 0)],
            vec![(0, 5, 0)],
            vec![(0, 0, 256)],
            vec![(0, 0, 0); 33],
        ] {
            assert!(parse(&data(&entries)).is_err());
        }
        let mut bad = bytes;
        bad[14] = 1;
        assert!(parse(&bad).is_err());
        assert!(parse(&data(&[])).unwrap().is_empty());
    }
}
