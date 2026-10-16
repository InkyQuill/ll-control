//! Scoped, bounded HTTP range responses for the current prepared-media review.
use crate::media_import::{MediaImports, MAX_CHUNK};
use tauri::http::{header, Method, Request, Response, StatusCode};

/// Opaque-id lookup shared by prepared review and editable gallery source.
pub trait PreviewSource {
    fn preview_length(&self, id: &str) -> anyhow::Result<u64>;
    fn preview_range(&self, id: &str, offset: u64, length: usize) -> anyhow::Result<Vec<u8>>;
}
impl PreviewSource for MediaImports {
    fn preview_length(&self, id: &str) -> anyhow::Result<u64> {
        MediaImports::preview_length(self, id)
    }
    fn preview_range(&self, id: &str, offset: u64, length: usize) -> anyhow::Result<Vec<u8>> {
        MediaImports::preview_range(self, id, offset, length)
    }
}
impl PreviewSource for crate::media_source::MediaSources {
    fn preview_length(&self, id: &str) -> anyhow::Result<u64> {
        Self::preview_length(self, id)
    }
    fn preview_range(&self, id: &str, offset: u64, length: usize) -> anyhow::Result<Vec<u8>> {
        Self::preview_range(self, id, offset, length)
    }
}

impl PreviewSource for crate::screen_record_jobs::Recordings {
    fn preview_length(&self, id: &str) -> anyhow::Result<u64> {
        Self::preview_length(self, id)
    }
    fn preview_range(&self, id: &str, offset: u64, length: usize) -> anyhow::Result<Vec<u8>> {
        Self::preview_range(self, id, offset, length)
    }
}

fn empty(status: StatusCode) -> Response<Vec<u8>> {
    let mut response = Response::new(Vec::new());
    *response.status_mut() = status;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response
}

/// Inclusive bounded range. Multipart ranges are deliberately unsupported.
pub(crate) fn requested_range(value: Option<&str>, total: u64) -> Option<(u64, u64)> {
    if total == 0 {
        return None;
    }
    let (start, end) = if let Some(value) = value {
        let value = value.strip_prefix("bytes=")?;
        let (start, end) = value.split_once('-')?;
        if start.is_empty() {
            let suffix: u64 = end.parse().ok()?;
            if suffix == 0 {
                return None;
            }
            (total.saturating_sub(suffix), total - 1)
        } else {
            let start: u64 = start.parse().ok()?;
            let end = if end.is_empty() {
                total - 1
            } else {
                end.parse::<u64>().ok()?.min(total - 1)
            };
            (start, end)
        }
    } else {
        (0, total - 1)
    };
    if start >= total || end < start {
        return None;
    }
    Some((start, end))
}

fn range(value: Option<&str>, total: u64) -> Option<(u64, u64)> {
    requested_range(value, total)
        .map(|(start, end)| (start, end.min(start.saturating_add(MAX_CHUNK as u64 - 1))))
}

/// Serve only a job ID, never a filesystem path. Run on a blocking worker.
/// Large/open ranges return a capped 206; browsers request subsequent ranges.
pub fn serve(
    imports: &impl PreviewSource,
    window: &str,
    request: Request<Vec<u8>>,
) -> Response<Vec<u8>> {
    if window != "main" {
        return empty(StatusCode::FORBIDDEN);
    }
    if request.method() != Method::GET && request.method() != Method::HEAD {
        let mut response = empty(StatusCode::METHOD_NOT_ALLOWED);
        response
            .headers_mut()
            .insert(header::ALLOW, "GET, HEAD".parse().unwrap());
        return response;
    }
    let id = request.uri().path().strip_prefix('/').unwrap_or("");
    if request.uri().query().is_some()
        || id.len() != 32
        || !id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return empty(StatusCode::NOT_FOUND);
    }
    let Ok(total) = imports.preview_length(id) else {
        return empty(StatusCode::NOT_FOUND);
    };
    let requested = request.headers().get(header::RANGE);
    let parsed = match requested {
        Some(value) => value.to_str().ok().and_then(|v| range(Some(v), total)),
        None => range(None, total),
    };
    let Some((start, end)) = parsed else {
        let mut response = empty(StatusCode::RANGE_NOT_SATISFIABLE);
        response.headers_mut().insert(
            header::CONTENT_RANGE,
            format!("bytes */{total}").parse().unwrap(),
        );
        return response;
    };
    let head = request.method() == Method::HEAD;
    let partial = !head && (requested.is_some() || end + 1 < total);
    let body = if head {
        Vec::new()
    } else {
        match imports.preview_range(id, start, (end - start + 1) as usize) {
            Ok(bytes) => bytes,
            // Confirmation/cancellation can revoke a preview between metadata and read.
            Err(_) => return empty(StatusCode::NOT_FOUND),
        }
    };
    let mut response = Response::new(body);
    *response.status_mut() = if partial {
        StatusCode::PARTIAL_CONTENT
    } else {
        StatusCode::OK
    };
    let headers = response.headers_mut();
    headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".parse().unwrap());
    headers.insert(header::CONTENT_TYPE, "video/mp4".parse().unwrap());
    headers.insert(header::ACCEPT_RANGES, "bytes".parse().unwrap());
    headers.insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    headers.insert(
        header::CONTENT_LENGTH,
        (if head { total } else { end - start + 1 })
            .to_string()
            .parse()
            .unwrap(),
    );
    if partial {
        headers.insert(
            header::CONTENT_RANGE,
            format!("bytes {start}-{end}/{total}").parse().unwrap(),
        );
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ranges_are_bounded_and_invalid_requests_do_not_read_files() {
        assert_eq!(range(Some("bytes=10-19"), 100), Some((10, 19)));
        assert_eq!(range(Some("bytes=98-999"), 100), Some((98, 99)));
        assert_eq!(range(Some("bytes=-7"), 100), Some((93, 99)));
        assert_eq!(range(Some("bytes=-999"), 100), Some((0, 99)));
        assert_eq!(
            range(Some("bytes=3-"), 1_000_000),
            Some((3, 3 + MAX_CHUNK as u64 - 1))
        );
        assert_eq!(range(None, 1_000_000), Some((0, MAX_CHUNK as u64 - 1)));
        for value in [
            "bytes=100-",
            "bytes=2-1",
            "bytes=-0",
            "bytes=0-1,4-5",
            "items=0-1",
            "bytes=18446744073709551616-",
            "bytes=1-2-3",
        ] {
            assert_eq!(range(Some(value), 100), None, "{value}");
        }
        let imports = MediaImports::default();
        let request = || {
            Request::builder()
                .uri("media-preview://localhost/../../etc/passwd")
                .body(Vec::new())
                .unwrap()
        };
        assert_eq!(
            serve(&imports, "other", request()).status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            serve(&imports, "main", request()).status(),
            StatusCode::NOT_FOUND
        );
    }
}
