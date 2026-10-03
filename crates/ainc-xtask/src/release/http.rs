//! A minimal HTTP/1.1 client and static file server for loopback use, so the smoke tests and
//! the upgrade gate need no HTTP dependency.
use anyhow::{Context, Result, anyhow, bail};
#[cfg(test)]
use std::thread;
use std::{
    io::{Read, Write},
    net::TcpStream,
    time::Duration,
};

fn decode_chunked(mut data: &[u8]) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    loop {
        let line_end = data
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or_else(|| anyhow!("bad chunked body"))?;
        let size = usize::from_str_radix(
            std::str::from_utf8(&data[..line_end])?
                .split(';')
                .next()
                .unwrap_or("")
                .trim(),
            16,
        )?;
        data = &data[line_end + 2..];
        if size == 0 {
            return Ok(body);
        }
        if data.len() < size + 2 {
            bail!("truncated chunked body");
        }
        body.extend_from_slice(&data[..size]);
        data = &data[size + 2..];
    }
}

/// One request to `http://host:port/path`; a status of 400 or more is an error, like urllib.
pub fn request(
    url: &str,
    method: &str,
    headers: &[(&str, String)],
    body: Option<&[u8]>,
    timeout: Duration,
) -> Result<Vec<u8>> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| anyhow!("only http:// URLs are supported: {url}"))?;
    let (authority, path) = rest
        .split_once('/')
        .map_or((rest, "/".to_string()), |(a, p)| (a, format!("/{p}")));
    let mut stream =
        TcpStream::connect(authority).with_context(|| format!("connect {authority}"))?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    let mut head =
        format!("{method} {path} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n");
    for (name, value) in headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    if let Some(body) = body {
        head.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    if let Some(body) = body {
        stream.write_all(body)?;
    }
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw)?;
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| anyhow!("malformed HTTP response"))?;
    let head = String::from_utf8_lossy(&raw[..split]).into_owned();
    let status: u16 = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse().ok())
        .ok_or_else(|| anyhow!("malformed HTTP status line"))?;
    let chunked = head.lines().skip(1).any(|line| {
        let line = line.to_ascii_lowercase();
        line.starts_with("transfer-encoding:") && line.contains("chunked")
    });
    let payload = &raw[split + 4..];
    let body = if chunked {
        decode_chunked(payload)?
    } else {
        payload.to_vec()
    };
    if status >= 400 {
        bail!("HTTP Error {status} for {url}");
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    /// Answers one request with `response`, returning the request text it saw.
    fn once(response: &'static [u8]) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut seen = Vec::new();
            let mut chunk = [0u8; 1024];
            // Read the whole request, body included: closing with unread data resets the
            // connection on Linux and the client then loses the response.
            loop {
                let end = seen.windows(4).position(|w| w == b"\r\n\r\n");
                if let Some(end) = end {
                    let head = String::from_utf8_lossy(&seen[..end]).to_ascii_lowercase();
                    let length = head
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length:"))
                        .and_then(|value| value.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    if seen.len() >= end + 4 + length {
                        break;
                    }
                }
                let n = stream.read(&mut chunk).unwrap();
                seen.extend_from_slice(&chunk[..n]);
            }
            stream.write_all(response).unwrap();
            String::from_utf8_lossy(&seen).into_owned()
        });
        (base, handle)
    }

    #[test]
    fn posts_json_and_reads_a_content_length_body() {
        let (base, seen) = once(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}");
        let headers = [("Authorization", "Bearer t".to_string())];
        let body = request(
            &format!("{base}/v1/state"),
            "POST",
            &headers,
            Some(b"{}"),
            Duration::from_secs(10),
        )
        .unwrap();
        assert_eq!(body, b"{}");
        let seen = seen.join().unwrap();
        assert!(seen.starts_with("POST /v1/state HTTP/1.1"));
        assert!(seen.contains("Authorization: Bearer t"));
    }

    #[test]
    fn reads_chunked_bodies_and_rejects_error_statuses() {
        let (base, _) =
            once(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n0\r\n\r\n");
        assert_eq!(
            request(&base, "GET", &[], None, Duration::from_secs(10)).unwrap(),
            b"abc"
        );
        let (base, _) = once(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
        assert!(
            request(
                &format!("{base}/x"),
                "GET",
                &[],
                None,
                Duration::from_secs(10)
            )
            .is_err()
        );
    }

    #[test]
    fn decodes_chunked_bodies() {
        assert_eq!(
            decode_chunked(b"3\r\nabc\r\n2\r\nde\r\n0\r\n\r\n").unwrap(),
            b"abcde"
        );
    }
}
