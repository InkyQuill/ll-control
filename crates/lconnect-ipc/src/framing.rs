//! Size-bounded newline framing for JSON-RPC connections.

use std::io::{Error, ErrorKind, Result};
use tokio::io::{AsyncBufRead, AsyncBufReadExt};

pub const MAX_JSON_RPC_LINE_BYTES: usize = 8 * 1024 * 1024;

/// Read one newline-terminated frame without ever accumulating more than
/// `limit` bytes. The returned length includes the newline, matching
/// `AsyncBufReadExt::read_line`; zero means clean EOF.
pub async fn read_bounded_line<R>(reader: &mut R, line: &mut String, limit: usize) -> Result<usize>
where
    R: AsyncBufRead + Unpin,
{
    line.clear();
    let mut total = 0usize;
    let mut bytes = Vec::new();
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            *line = String::from_utf8(bytes)
                .map_err(|error| Error::new(ErrorKind::InvalidData, error))?;
            return Ok(total);
        }
        let take = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(available.len(), |position| position + 1);
        if total.saturating_add(take) > limit {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("JSON-RPC line exceeds {limit} bytes"),
            ));
        }
        bytes.extend_from_slice(&available[..take]);
        reader.consume(take);
        total += take;
        if bytes.last() == Some(&b'\n') {
            *line = String::from_utf8(bytes)
                .map_err(|error| Error::new(ErrorKind::InvalidData, error))?;
            return Ok(total);
        }
    }
}
