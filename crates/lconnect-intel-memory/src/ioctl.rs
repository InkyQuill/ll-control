//! The only unsafe boundary: fixed read-only DRM requests, pinned live buffers.
use crate::{engines, regions, Driver, Engine, Memory, QueryError};
use std::{
    fs::OpenOptions,
    os::{
        fd::AsRawFd,
        unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt},
    },
    path::Path,
};

#[repr(C)]
struct Version {
    major: i32,
    minor: i32,
    patch: i32,
    name_len: usize,
    name: *mut libc::c_char,
    date_len: usize,
    date: *mut libc::c_char,
    desc_len: usize,
    desc: *mut libc::c_char,
}
#[repr(C)]
struct I915Item {
    id: u64,
    length: i32,
    flags: u32,
    data: u64,
}
#[repr(C)]
struct I915Query {
    count: u32,
    flags: u32,
    items: u64,
}
#[repr(C)]
struct XeQuery {
    extensions: u64,
    query: u32,
    size: u32,
    data: u64,
    reserved: [u64; 2],
}

// UAPI structure layout is independent of native pointer size except VERSION.
const _: () = assert!(std::mem::size_of::<I915Item>() == 24);
const _: () = assert!(std::mem::size_of::<I915Query>() == 16);
const _: () = assert!(std::mem::size_of::<XeQuery>() == 40);

// SAFETY: caller must choose the UAPI number for T and keep all pointed-to
// storage writable and alive through the synchronous ioctl. Never public.
unsafe fn call<T>(fd: i32, number: u32, value: &mut T) -> Result<(), QueryError> {
    // SAFETY: the private callers below satisfy the fixed UAPI contract.
    let result = unsafe { libc::ioctl(fd, libc::_IOWR::<T>(b'd' as u32, number), value as *mut T) };
    if result < 0 {
        Err(QueryError::Unavailable)
    } else {
        Ok(())
    }
}

fn open_verified(
    path: &Path,
    expected_dev: u64,
    driver: Driver,
) -> Result<std::fs::File, QueryError> {
    // DRM render nodes accept read-only queries on an O_RDONLY descriptor.
    // O_NOFOLLOW avoids substituted symlinks; fstat checks the opened object.
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| QueryError::Unavailable)?;
    let stat = file.metadata().map_err(|_| QueryError::Unavailable)?;
    if !stat.file_type().is_char_device() || stat.rdev() != expected_dev {
        return Err(QueryError::Unavailable);
    }
    let fd = file.as_raw_fd();
    let mut name = [0u8; 16];
    let mut version = Version {
        major: 0,
        minor: 0,
        patch: 0,
        name_len: name.len(),
        name: name.as_mut_ptr().cast(),
        date_len: 0,
        date: std::ptr::null_mut(),
        desc_len: 0,
        desc: std::ptr::null_mut(),
    };
    // SAFETY: VERSION 0x00 uses this repr(C) structure and writable name buffer;
    // optional date/description have zero lengths. Both outlive the ioctl.
    unsafe { call(fd, 0x00, &mut version) }?;
    if version.name_len > name.len() || &name[..version.name_len] != driver.name().as_bytes() {
        return Err(QueryError::Unavailable);
    }
    Ok(file)
}
pub(crate) fn query_engines(path: &Path, expected_dev: u64) -> Result<Vec<Engine>, QueryError> {
    let file = open_verified(path, expected_dev, Driver::Xe)?;
    let buffer = query_buffer(engines::MAX_BYTES, |buffer| {
        xe_query(file.as_raw_fd(), 0, buffer)
    })?;
    engines::parse(&buffer)
}
fn xe_query(fd: i32, kind: u32, buffer: Option<&mut [u8]>) -> Result<usize, QueryError> {
    let length = buffer.as_ref().map_or(0, |b| b.len());
    let pointer = buffer.map_or(0, |b| b.as_mut_ptr() as u64);
    let mut query = XeQuery {
        extensions: 0,
        query: kind,
        size: length as u32,
        data: pointer,
        reserved: [0; 2],
    };
    // SAFETY: XE_DEVICE_QUERY 0x40 with ENGINES=0 or MEM_REGIONS=1, private
    // callers only. Data points to length writable bytes or null for size probe.
    unsafe { call(fd, 0x40, &mut query) }?;
    Ok(query.size as usize)
}
pub(crate) fn query(path: &Path, expected_dev: u64, driver: Driver) -> Result<Memory, QueryError> {
    let file = open_verified(path, expected_dev, driver)?;
    let fd = file.as_raw_fd();
    query_memory(driver, |buffer| {
        let length = buffer.as_ref().map_or(0, |b| b.len());
        let pointer = buffer.map_or(0, |b| b.as_mut_ptr() as u64);
        match driver {
            Driver::I915 => {
                let mut item = I915Item {
                    id: 4,
                    length: length as i32,
                    flags: 0,
                    data: pointer,
                };
                let mut query = I915Query {
                    count: 1,
                    flags: 0,
                    items: (&mut item as *mut I915Item) as u64,
                };
                // SAFETY: I915_QUERY 0x79, exactly one live item, zero flags.
                // Null data on size probe; second pass is bounded to buffer.len().
                unsafe { call(fd, 0x79, &mut query) }?;
                usize::try_from(item.length).map_err(|_| QueryError::Unavailable)
            }
            Driver::Xe => {
                let mut query = XeQuery {
                    extensions: 0,
                    query: 1,
                    size: length as u32,
                    data: pointer,
                    reserved: [0; 2],
                };
                // SAFETY: XE_DEVICE_QUERY 0x40 with MEM_REGIONS=1. Buffer has
                // query.size bytes and remains live until this synchronous call.
                unsafe { call(fd, 0x40, &mut query) }?;
                Ok(query.size as usize)
            }
        }
    })
}
fn query_memory(
    driver: Driver,
    read: impl FnMut(Option<&mut [u8]>) -> Result<usize, QueryError>,
) -> Result<Memory, QueryError> {
    let buffer = query_buffer(regions::MAX_BYTES, read)?;
    regions::parse(driver, &buffer)
}
fn query_buffer(
    max_bytes: usize,
    mut read: impl FnMut(Option<&mut [u8]>) -> Result<usize, QueryError>,
) -> Result<Vec<u8>, QueryError> {
    let size = read(None)?;
    if size == 0 || size > max_bytes {
        return Err(QueryError::Invalid);
    }
    let mut buffer = vec![0; size];
    if read(Some(&mut buffer))? != size {
        return Err(QueryError::Invalid);
    }
    Ok(buffer)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn query_size_bounds_and_changed_size_fail_before_parsing() {
        for size in [0, regions::MAX_BYTES + 1, usize::MAX] {
            let mut calls = 0;
            assert_eq!(
                query_memory(Driver::Xe, |_| {
                    calls += 1;
                    Ok(size)
                }),
                Err(QueryError::Invalid)
            );
            assert_eq!(calls, 1);
        }
        assert_eq!(
            query_memory(Driver::I915, |_| Err(QueryError::Unavailable)),
            Err(QueryError::Unavailable)
        );
        assert_eq!(
            query_memory(Driver::Xe, |buf| Ok(if buf.is_none() { 96 } else { 184 })),
            Err(QueryError::Invalid)
        );
    }
    #[test]
    fn ordinary_files_cannot_reach_ioctl() {
        let file = tempfile::NamedTempFile::new().unwrap();
        assert_eq!(
            query(file.path(), libc::makedev(226, 128), Driver::Xe),
            Err(QueryError::Unavailable)
        );
    }
}
