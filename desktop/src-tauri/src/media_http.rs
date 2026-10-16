//! Loopback delivery for WebKitGTK, whose GStreamer source does not support
//! arbitrary URI schemes. Only current managed media IDs are addressable.
use crate::{media_import::MediaImports, media_preview, media_source::MediaSources};
use crate::{media_preview::PreviewSource, screen_record_jobs::Recordings};
use anyhow::{bail, Context, Result};
use std::{
    os::fd::{AsFd, AsRawFd},
    sync::Arc,
    time::Duration,
};
use tauri::http::{header, Method, Request, StatusCode};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{watch, Mutex},
    task::{JoinHandle, JoinSet},
};

const HEADER_LIMIT: usize = 8192;
const CLIENT_LIMIT: usize = 4;
const IO_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Default)]
pub struct MediaHttp(Arc<Mutex<State>>);
#[derive(Default)]
struct State {
    server: Option<Server>,
    closed: bool,
}
struct Server {
    base: String,
    stop: watch::Sender<bool>,
    task: JoinHandle<Result<()>>,
}
impl MediaHttp {
    pub async fn url(
        &self,
        kind: &str,
        id: &str,
        imports: MediaImports,
        sources: MediaSources,
        recordings: Recordings,
    ) -> Result<String> {
        if !matches!(kind, "review" | "source" | "recording")
            || id.len() != 32
            || !id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            bail!("Invalid media identity");
        }
        let mut state = self.0.lock().await;
        if state.closed {
            bail!("Media server is closing");
        }
        if state.server.is_none() {
            let listener = TcpListener::bind("127.0.0.1:0").await?;
            let authority = listener.local_addr()?.to_string();
            let token = uuid::Uuid::new_v4().simple().to_string();
            let base = format!("http://{authority}/{token}");
            let (stop, mut stopping) = watch::channel(false);
            let task = tokio::spawn(async move {
                let mut clients = JoinSet::new();
                loop {
                    tokio::select! {
                        biased;
                        _ = stopping.changed() => break,
                        _ = clients.join_next(), if !clients.is_empty() => {},
                        accepted = listener.accept(), if clients.len() < CLIENT_LIMIT => {
                            let Ok((stream, _)) = accepted else { break };
                            let imports = imports.clone();
                            let sources = sources.clone();
                            let recordings = recordings.clone();
                            let token = token.clone();
                            let authority = authority.clone();
                            let stopping = stopping.clone();
                            clients.spawn(async move {
                                let _ = client(stream, &authority, &token, imports, sources, recordings, stopping).await;
                            });
                        }
                    }
                }
                let closed = stop_listener(&listener);
                drop(listener);
                // Every network operation has a deadline. Blocking file reads remain
                // owned and joined even when the window closes or the peer leaves.
                while clients.join_next().await.is_some() {}
                closed
            });
            state.server = Some(Server { base, stop, task });
        }
        Ok(format!(
            "{}/{kind}/{id}",
            state.server.as_ref().unwrap().base
        ))
    }
    pub async fn shutdown(&self) -> Result<()> {
        let mut state = self.0.lock().await;
        state.closed = true;
        if let Some(server) = state.server.take() {
            let _ = server.stop.send(true);
            server.task.await.context("Media server task")??;
        }
        Ok(())
    }
}

// Closing one descriptor does not stop a listener while another descriptor
// refers to it (including a child between fork and exec). Stop the socket
// itself before releasing our descriptor, then join the existing clients.
fn stop_listener(listener: &impl AsFd) -> Result<()> {
    // SAFETY: the borrowed descriptor remains alive for this non-owning syscall.
    let result = unsafe { libc::shutdown(listener.as_fd().as_raw_fd(), libc::SHUT_RDWR) };
    if result != 0 {
        return Err(std::io::Error::last_os_error()).context("Stop media listener");
    }
    Ok(())
}

fn parse(bytes: &[u8], authority: &str, token: &str) -> Result<(String, Request<Vec<u8>>)> {
    let mut headers = [httparse::EMPTY_HEADER; 32];
    let mut parsed = httparse::Request::new(&mut headers);
    if !parsed.parse(bytes)?.is_complete() {
        bail!("Incomplete media request");
    }
    let mut request = Request::builder().method(parsed.method.context("Missing method")?);
    let path = parsed.path.context("Missing path")?;
    let mut pieces = path.split('/');
    if pieces.next() != Some("") || pieces.next() != Some(token) {
        bail!("Unknown media token");
    }
    let kind = pieces.next().context("Missing media kind")?;
    let id = pieces.next().context("Missing media ID")?;
    if !matches!(kind, "review" | "source" | "recording") || pieces.next().is_some() {
        bail!("Invalid media route");
    }
    let mut host = None;
    let mut range = false;
    for h in parsed.headers {
        if h.name.eq_ignore_ascii_case("host") {
            if host.replace(h.value).is_some() {
                bail!("Duplicate Host");
            }
        } else if h.name.eq_ignore_ascii_case("origin") {
            if ![
                b"tauri://localhost".as_slice(),
                b"http://tauri.localhost",
                b"https://tauri.localhost",
                b"http://localhost:1420",
                b"http://127.0.0.1:1420",
            ]
            .contains(&h.value)
            {
                bail!("Invalid media origin");
            }
        } else if h.name.eq_ignore_ascii_case("transfer-encoding")
            || (h.name.eq_ignore_ascii_case("content-length") && h.value != b"0")
        {
            bail!("Media requests have no body");
        } else if h.name.eq_ignore_ascii_case("range") {
            if range {
                bail!("Duplicate Range");
            }
            range = true;
            request = request.header(header::RANGE, h.value);
        }
    }
    if host != Some(authority.as_bytes()) {
        bail!("Invalid media host");
    }
    Ok((
        kind.to_owned(),
        request.uri(format!("/{id}")).body(Vec::new())?,
    ))
}

async fn client(
    mut stream: TcpStream,
    authority: &str,
    token: &str,
    imports: MediaImports,
    sources: MediaSources,
    recordings: Recordings,
    stopping: watch::Receiver<bool>,
) -> Result<()> {
    let bytes = tokio::time::timeout(IO_TIMEOUT, async {
        let mut bytes = Vec::new();
        loop {
            let mut chunk = [0u8; 1024];
            let read = stream.read(&mut chunk).await?;
            if read == 0 {
                bail!("Incomplete request");
            }
            bytes.extend_from_slice(&chunk[..read]);
            if bytes.len() > HEADER_LIMIT {
                bail!("Media header too large");
            }
            if bytes.windows(4).any(|w| w == b"\r\n\r\n") {
                return Ok::<_, anyhow::Error>(bytes);
            }
        }
    })
    .await??;
    let (kind, request) = parse(&bytes, authority, token)?;
    if kind == "review" {
        respond(stream, Arc::new(imports), request, stopping).await
    } else if kind == "source" {
        respond(stream, Arc::new(sources), request, stopping).await
    } else {
        respond(stream, Arc::new(recordings), request, stopping).await
    }
}

async fn respond<S: PreviewSource + Send + Sync + 'static>(
    mut stream: TcpStream,
    source: Arc<S>,
    request: Request<Vec<u8>>,
    stopping: watch::Receiver<bool>,
) -> Result<()> {
    let owner = source.clone();
    let (response, remainder) = tokio::task::spawn_blocking(move || -> Result<_> {
        // Reuse opaque-ID/method/range admission and first bounded file read.
        let response = media_preview::serve(owner.as_ref(), "main", request.clone());
        if request.method() != Method::GET || !response.status().is_success() {
            return Ok((response, None));
        }
        let id = request.uri().path().trim_start_matches('/').to_owned();
        let total = owner.preview_length(&id)?;
        let requested = request.headers().get(header::RANGE);
        let (start, end) =
            media_preview::requested_range(requested.and_then(|v| v.to_str().ok()), total)
                .context("Invalid stream range")?;
        let mut response = response;
        *response.status_mut() = if requested.is_some() {
            StatusCode::PARTIAL_CONTENT
        } else {
            StatusCode::OK
        };
        response.headers_mut().insert(
            header::CONTENT_LENGTH,
            (end - start + 1).to_string().parse()?,
        );
        if requested.is_some() {
            response.headers_mut().insert(
                header::CONTENT_RANGE,
                format!("bytes {start}-{end}/{total}").parse()?,
            );
        } else {
            response.headers_mut().remove(header::CONTENT_RANGE);
        }
        let offset = start + response.body().len() as u64;
        Ok((response, Some((id, offset, end))))
    })
    .await??;
    let mut head = format!("HTTP/1.1 {}\r\nConnection: close\r\n", response.status());
    for (name, value) in response.headers() {
        head.push_str(name.as_str());
        head.push_str(": ");
        head.push_str(value.to_str()?);
        head.push_str("\r\n");
    }
    if !response.headers().contains_key(header::CONTENT_LENGTH) {
        head.push_str("content-length: 0\r\n");
    }
    head.push_str("\r\n");
    tokio::time::timeout(IO_TIMEOUT, stream.write_all(head.as_bytes())).await??;
    tokio::time::timeout(IO_TIMEOUT, stream.write_all(response.body())).await??;
    // Release the first allocation before reading the next chunk. Full/open HTTP
    // ranges are streamed, never silently shortened to the disk-read cap.
    drop(response);
    if let Some((id, mut offset, end)) = remainder {
        while offset <= end {
            if *stopping.borrow() {
                bail!("Media server closing");
            }
            let length = (end - offset + 1).min(crate::media_import::MAX_CHUNK as u64) as usize;
            let owner = source.clone();
            let key = id.clone();
            let bytes =
                tokio::task::spawn_blocking(move || owner.preview_range(&key, offset, length))
                    .await??;
            if bytes.len() != length {
                bail!("Incomplete media read");
            }
            tokio::time::timeout(IO_TIMEOUT, stream.write_all(&bytes)).await??;
            offset += length as u64;
        }
    }
    stream.shutdown().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Bytes {
        data: Vec<u8>,
        revoke_after_first: bool,
    }
    impl PreviewSource for Bytes {
        fn preview_length(&self, _: &str) -> Result<u64> {
            Ok(self.data.len() as u64)
        }
        fn preview_range(&self, _: &str, offset: u64, length: usize) -> Result<Vec<u8>> {
            assert!(length <= crate::media_import::MAX_CHUNK);
            if self.revoke_after_first && offset >= crate::media_import::MAX_CHUNK as u64 {
                bail!("Revoked");
            }
            Ok(self.data[offset as usize..offset as usize + length].to_vec())
        }
    }
    async fn transfer(range: Option<&str>, revoked: bool) -> (Vec<u8>, bool) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut peer = TcpStream::connect(listener.local_addr().unwrap())
            .await
            .unwrap();
        let (stream, _) = listener.accept().await.unwrap();
        let source = Arc::new(Bytes {
            data: (0..700_000).map(|i| (i % 251) as u8).collect(),
            revoke_after_first: revoked,
        });
        let mut request = Request::builder().uri(format!("/{}", "a".repeat(32)));
        if let Some(range) = range {
            request = request.header(header::RANGE, range);
        }
        let (_stop, stopping) = watch::channel(false);
        let task = tokio::spawn(respond(
            stream,
            source,
            request.body(Vec::new()).unwrap(),
            stopping,
        ));
        let mut response = Vec::new();
        peer.read_to_end(&mut response).await.unwrap();
        (response, task.await.unwrap().is_ok())
    }
    #[tokio::test]
    async fn full_and_open_ranges_stream_beyond_read_cap_without_truncation() {
        for (range, start) in [(None, 0), (Some("bytes=123-"), 123)] {
            let (response, ok) = transfer(range, false).await;
            assert!(ok);
            let boundary = response.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
            let head = std::str::from_utf8(&response[..boundary]).unwrap();
            assert!(head.starts_with(if range.is_some() {
                "HTTP/1.1 206"
            } else {
                "HTTP/1.1 200"
            }));
            assert!(head.contains(&format!("content-length: {}", 700_000 - start)));
            let expected: Vec<u8> = (start..700_000).map(|i| (i % 251) as u8).collect();
            assert_eq!(&response[boundary..], expected);
        }
    }
    #[tokio::test]
    async fn revoked_owner_closes_stream_instead_of_reading_stale_content() {
        let (response, ok) = transfer(None, true).await;
        assert!(!ok);
        let boundary = response.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
        assert_eq!(response.len() - boundary, crate::media_import::MAX_CHUNK);
    }
    #[test]
    fn request_cannot_escape_opaque_route_or_rebind_host() {
        let good = b"GET /secret/review/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa HTTP/1.1\r\nHost: 127.0.0.1:42\r\nRange: bytes=5-9\r\nOrigin: tauri://localhost\r\n\r\n";
        let (_, request) = parse(good, "127.0.0.1:42", "secret").unwrap();
        assert_eq!(request.headers()[header::RANGE], "bytes=5-9");
        for (from, to) in [
            ("secret", "wrong"),
            ("127.0.0.1:42", "evil.example"),
            ("tauri://localhost", "https://evil.example"),
            ("review/", "other/"),
            ("Range: bytes=5-9", "Content-Length: 5"),
        ] {
            let bad = String::from_utf8(good.to_vec()).unwrap().replace(from, to);
            assert!(parse(bad.as_bytes(), "127.0.0.1:42", "secret").is_err());
        }
    }
    #[tokio::test]
    async fn stop_disables_listener_even_while_another_descriptor_retains_it() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let retained = listener.as_fd().try_clone_to_owned().unwrap();
        drop(listener);
        // A close alone leaves the kernel listener active.
        let connection = TcpStream::connect(address).await.unwrap();
        stop_listener(&retained).unwrap();
        assert!(TcpStream::connect(address).await.is_err());
        drop(connection);
        drop(retained);
    }
    #[tokio::test]
    async fn unavailable_owner_returns_not_found_and_shutdown_closes_listener() {
        let server = MediaHttp::default();
        let url = server
            .url(
                "review",
                &"a".repeat(32),
                MediaImports::default(),
                MediaSources::default(),
                Recordings::default(),
            )
            .await
            .unwrap();
        let uri: tauri::http::Uri = url.parse().unwrap();
        let authority = uri.authority().unwrap().as_str();
        let mut socket = TcpStream::connect(authority).await.unwrap();
        socket
            .write_all(
                format!("GET {} HTTP/1.1\r\nHost: {authority}\r\n\r\n", uri.path()).as_bytes(),
            )
            .await
            .unwrap();
        let mut response = String::new();
        socket.read_to_string(&mut response).await.unwrap();
        assert!(response.starts_with("HTTP/1.1 404"));
        server.shutdown().await.unwrap();
        if let Ok(socket) = TcpStream::connect(authority).await {
            panic!(
                "post-shutdown connection local={:?} peer={:?}",
                socket.local_addr(),
                socket.peer_addr()
            );
        }
        assert!(server
            .url(
                "review",
                &"a".repeat(32),
                MediaImports::default(),
                MediaSources::default(),
                Recordings::default(),
            )
            .await
            .is_err());
    }
}
