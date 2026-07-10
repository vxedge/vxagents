//! Shared plumbing for vxedge measurement agents.
//!
//! Lightweight is a requirement (AGENTS.md): no allocations on hot paths,
//! no heavyweight metrics frameworks — Prometheus text exposition is
//! hand-rendered, and the exporter is a minimal HTTP responder.

pub mod packet {
    //! Probe packet: 8-byte big-endian sequence number, zero-padded to the
    //! requested payload size. Reflectors echo bytes verbatim, so the sender
    //! correlates replies by decoding its own sequence number.

    pub const MIN_PAYLOAD: usize = 8;

    /// Write `seq` into `buf` (which must be at least MIN_PAYLOAD long).
    pub fn encode(seq: u64, buf: &mut [u8]) {
        buf[..8].copy_from_slice(&seq.to_be_bytes());
    }

    /// Read the sequence number back from an echoed packet.
    pub fn decode(buf: &[u8]) -> Option<u64> {
        buf.get(..8)
            .map(|b| u64::from_be_bytes(b.try_into().expect("8-byte slice")))
    }
}

pub mod prom {
    //! Minimal Prometheus text-format rendering helpers.

    /// Render one metric line with optional labels.
    pub fn line(out: &mut String, name: &str, labels: &[(&str, &str)], value: f64) {
        out.push_str(name);
        if !labels.is_empty() {
            out.push('{');
            for (i, (k, v)) in labels.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(k);
                out.push_str("=\"");
                out.push_str(v);
                out.push('"');
            }
            out.push('}');
        }
        out.push(' ');
        out.push_str(&format!("{value}"));
        out.push('\n');
    }
}

pub mod exporter {
    //! Minimal HTTP /metrics responder: accepts a connection, ignores the
    //! request beyond draining one read, answers with the rendered metrics.
    //! Good enough for Prometheus scrapes; not a general web server.

    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Serve `render()` output on `addr` until the task is dropped.
    pub async fn serve<F>(addr: std::net::SocketAddr, render: Arc<F>) -> std::io::Result<()>
    where
        F: Fn() -> String + Send + Sync + 'static,
    {
        let listener = TcpListener::bind(addr).await?;
        tracing::info!(%addr, "metrics exporter listening");
        loop {
            let (mut stream, _) = listener.accept().await?;
            let render = render.clone();
            tokio::spawn(async move {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf).await;
                let body = render();
                let resp = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: text/plain; version=0.0.4\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(resp.as_bytes()).await;
                let _ = stream.shutdown().await;
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_roundtrip() {
        let mut buf = vec![0u8; 64];
        packet::encode(0xDEAD_BEEF_0042, &mut buf);
        assert_eq!(packet::decode(&buf), Some(0xDEAD_BEEF_0042));
    }

    #[test]
    fn prom_line_renders_labels() {
        let mut s = String::new();
        prom::line(&mut s, "vxprobe_rtt_seconds", &[("q", "0.99")], 0.00042);
        assert_eq!(s, "vxprobe_rtt_seconds{q=\"0.99\"} 0.00042\n");
    }
}
