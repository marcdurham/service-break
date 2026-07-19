//! Ships structured logs to an OpenObserve instance
//! (<https://openobserve.ai/>) over its HTTP `_json` bulk ingestion
//! endpoint, alongside the existing stdout logging.
//!
//! Wiring: a `tracing_subscriber::fmt` JSON layer is pointed at a
//! [`ChannelWriter`] instead of stdout, so every log line (and, with
//! `tracing-actix-web`'s per-request root span, every REST request) that
//! already goes to the terminal is also serialized as one JSON object and
//! handed to an unbounded channel. A single background task drains that
//! channel, batches lines, and POSTs them to OpenObserve. Entirely disabled
//! (falls back to stdout-only logging) unless `OPENOBSERVE_URL` is set.
//!
//! The shipper never uses `tracing::*` macros itself — that would feed its
//! own errors back into the channel it's draining.

use std::io;
use std::time::Duration;

use tokio::sync::mpsc::{self, UnboundedSender};

/// How many buffered lines to send in one POST at most.
const BATCH_SIZE: usize = 200;
/// Longest a partial batch waits before being sent anyway.
const FLUSH_INTERVAL: Duration = Duration::from_secs(2);

pub struct OpenObserveConfig {
    ingest_url: String,
    user: String,
    password: String,
}

impl OpenObserveConfig {
    /// Reads `OPENOBSERVE_URL` (e.g. `http://openobserve:5080`),
    /// `OPENOBSERVE_ORG`, `OPENOBSERVE_STREAM`, `OPENOBSERVE_USER`, and
    /// `OPENOBSERVE_PASSWORD` from the environment. `None` when
    /// `OPENOBSERVE_URL` is unset — log shipping stays off.
    pub fn from_env() -> Option<Self> {
        let base = std::env::var("OPENOBSERVE_URL").ok()?;
        let base = base.trim_end_matches('/');
        let org = std::env::var("OPENOBSERVE_ORG").unwrap_or_else(|_| "default".to_owned());
        let stream = std::env::var("OPENOBSERVE_STREAM").unwrap_or_else(|_| "backend".to_owned());
        let user =
            std::env::var("OPENOBSERVE_USER").unwrap_or_else(|_| "admin@example.com".to_owned());
        let password = std::env::var("OPENOBSERVE_PASSWORD").unwrap_or_default();
        Some(Self { ingest_url: format!("{base}/api/{org}/{stream}/_json"), user, password })
    }
}

/// One per formatted log line; buffers the bytes `tracing_subscriber`'s
/// JSON formatter writes for a single event/span-close and forwards them
/// as one message when dropped (i.e. once that line is complete).
pub struct ChannelWriter {
    tx: UnboundedSender<Vec<u8>>,
    buf: Vec<u8>,
}

impl io::Write for ChannelWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buf.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for ChannelWriter {
    fn drop(&mut self) {
        if !self.buf.is_empty() {
            let _ = self.tx.send(std::mem::take(&mut self.buf));
        }
    }
}

#[derive(Clone)]
pub struct ChannelMakeWriter {
    tx: UnboundedSender<Vec<u8>>,
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for ChannelMakeWriter {
    type Writer = ChannelWriter;

    fn make_writer(&'a self) -> Self::Writer {
        ChannelWriter { tx: self.tx.clone(), buf: Vec::new() }
    }
}

/// Spawns the background batching/shipping task and returns the
/// [`ChannelMakeWriter`] to plug into a `tracing_subscriber::fmt` JSON
/// layer. Must be called from within a running Tokio runtime.
pub fn spawn_shipper(config: OpenObserveConfig, http: reqwest::Client) -> ChannelMakeWriter {
    let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();

    tokio::spawn(async move {
        let mut batch: Vec<u8> = Vec::new();
        let mut count = 0usize;
        loop {
            let recv = tokio::time::timeout(FLUSH_INTERVAL, rx.recv()).await;
            match recv {
                Ok(Some(line)) => {
                    batch.extend_from_slice(&line);
                    count += 1;
                    if count >= BATCH_SIZE {
                        flush(&http, &config, &mut batch, &mut count).await;
                    }
                }
                Ok(None) => {
                    flush(&http, &config, &mut batch, &mut count).await;
                    break;
                }
                Err(_timeout) => {
                    flush(&http, &config, &mut batch, &mut count).await;
                }
            }
        }
    });

    ChannelMakeWriter { tx }
}

/// POSTs newline-delimited JSON lines to OpenObserve's `_json` bulk
/// ingestion endpoint. Failures are only `eprintln!`'d — never routed back
/// through `tracing`, which would re-enter this same channel.
async fn flush(http: &reqwest::Client, config: &OpenObserveConfig, batch: &mut Vec<u8>, count: &mut usize) {
    if batch.is_empty() {
        return;
    }
    // OpenObserve's `_json` endpoint takes a JSON array; each buffered line
    // is already one JSON object, so wrap them in `[...]` with commas.
    let mut body = Vec::with_capacity(batch.len() + *count + 2);
    body.push(b'[');
    for (i, line) in batch.split(|&b| b == b'\n').filter(|l| !l.is_empty()).enumerate() {
        if i > 0 {
            body.push(b',');
        }
        body.extend_from_slice(line);
    }
    body.push(b']');

    let result = http
        .post(&config.ingest_url)
        .basic_auth(&config.user, Some(&config.password))
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await;
    match result {
        Ok(resp) if !resp.status().is_success() => {
            eprintln!("openobserve: ingest returned {}", resp.status());
        }
        Err(err) => eprintln!("openobserve: ingest failed: {err}"),
        Ok(_) => {}
    }

    batch.clear();
    *count = 0;
}
