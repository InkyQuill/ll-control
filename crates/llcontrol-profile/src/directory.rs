//! Bound the central directory before the ZIP library allocates its index.
use crate::{Error, Limits};
use std::io::{Read, Seek, SeekFrom};

pub(crate) struct Directory {
    pub entries: u64,
    pub offset: u64,
}
fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}
fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("fixed header"))
}
fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("fixed header"))
}
pub(crate) fn preflight<R: Read + Seek>(
    reader: &mut R,
    limits: Limits,
) -> Result<Directory, Error> {
    let length = reader.seek(SeekFrom::End(0))?;
    if length < 22 || length > limits.archive_bytes {
        return Err(Error::Invalid("ZIP file size is outside inspection limits"));
    }
    reader.seek(SeekFrom::Start(0))?;
    let mut signature = [0; 4];
    reader.read_exact(&mut signature)?;
    if signature != *b"PK\x03\x04" {
        return Err(Error::Invalid(
            "expected a Windows ZIP, not a self-extracting or empty archive",
        ));
    }
    reader.seek(SeekFrom::End(-22))?;
    let mut end = [0; 22];
    reader.read_exact(&mut end)?;
    // .NET's export does not set archive comments. Reject trailing/polyglot data.
    if end[..4] != *b"PK\x05\x06" || u16_at(&end, 20) != 0 {
        return Err(Error::Invalid(
            "ZIP comments or trailing data are not supported",
        ));
    }
    if u16_at(&end, 4) != 0 || u16_at(&end, 6) != 0 {
        return Err(Error::Invalid("multi-disk ZIP is not supported"));
    }
    let entries16 = u16_at(&end, 10);
    if u16_at(&end, 8) != entries16 {
        return Err(Error::Invalid("ZIP entry counts disagree"));
    }
    let (entries, size, offset, boundary) = if entries16 == u16::MAX
        || u32_at(&end, 12) == u32::MAX
        || u32_at(&end, 16) == u32::MAX
    {
        if length < 98 {
            return Err(Error::Invalid("truncated ZIP64 directory"));
        }
        reader.seek(SeekFrom::End(-42))?;
        let mut locator = [0; 20];
        reader.read_exact(&mut locator)?;
        if locator[..4] != *b"PK\x06\x07" || u32_at(&locator, 4) != 0 || u32_at(&locator, 16) != 1 {
            return Err(Error::Invalid("invalid ZIP64 locator"));
        }
        let position = u64_at(&locator, 8);
        if position > length - 98 {
            return Err(Error::Invalid("ZIP64 directory is outside file"));
        }
        reader.seek(SeekFrom::Start(position))?;
        let mut header = [0; 56];
        reader.read_exact(&mut header)?;
        let header_size = u64_at(&header, 4);
        if header[..4] != *b"PK\x06\x06"
            || !(44..=1024).contains(&header_size)
            || position.checked_add(12 + header_size) != Some(length - 42)
            || u32_at(&header, 16) != 0
            || u32_at(&header, 20) != 0
            || u64_at(&header, 24) != u64_at(&header, 32)
        {
            return Err(Error::Invalid("invalid ZIP64 directory"));
        }
        (
            u64_at(&header, 32),
            u64_at(&header, 40),
            u64_at(&header, 48),
            position,
        )
    } else {
        (
            u64::from(entries16),
            u64::from(u32_at(&end, 12)),
            u64::from(u32_at(&end, 16)),
            length - 22,
        )
    };
    if entries == 0
        || entries > limits.entries as u64
        || size > limits.directory_bytes
        || size < entries.saturating_mul(46)
        || offset.checked_add(size) != Some(boundary)
    {
        return Err(Error::Invalid(
            "ZIP directory exceeds bounds or has inconsistent extent",
        ));
    }
    reader.seek(SeekFrom::Start(0))?;
    Ok(Directory { entries, offset })
}
