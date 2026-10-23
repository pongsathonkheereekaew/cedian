//! A DevTools connection to one page of the shared browser, for the Browser block: the page's
//! screencast comes in as frames, and the person's clicks, keys and navigation go out as calls.
//! It runs on a thread of its own, which also decodes each frame and answers it, so a slow
//! window only makes Chromium send fewer frames.

use std::{
    io::ErrorKind,
    net::TcpStream,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::Duration,
};

use anyhow::{Context as _, anyhow};
use base64::Engine as _;
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use gpui::RenderImage;
use serde_json::{Value, json};
use tungstenite::{Message, WebSocket};

/// How often the thread looks for calls to send while it waits for the page.
const POLL: Duration = Duration::from_millis(15);

pub enum CdpEvent {
    /// A screencast frame, decoded, with the page's size in CSS pixels.
    Frame {
        image: Arc<RenderImage>,
        width: f32,
        height: f32,
    },
    /// An event from the page, or the answer to a call (with its `id`).
    Message(Value),
    /// The connection ended, and why.
    Closed(String),
}

/// Sends calls to the page. Dropping it ends the connection.
pub struct Cdp {
    calls: mpsc::Sender<String>,
    next: Arc<AtomicU64>,
}

impl Cdp {
    /// Connects to a page's `ws://host:port/devtools/page/<id>`.
    pub fn connect(socket: &str) -> anyhow::Result<(Self, UnboundedReceiver<CdpEvent>)> {
        let host = socket
            .strip_prefix("ws://")
            .and_then(|rest| rest.split('/').next())
            .ok_or_else(|| anyhow!("not a page socket: {socket}"))?;
        let stream = TcpStream::connect(host).with_context(|| format!("connecting to {host}"))?;
        let (mut ws, _) = tungstenite::client(socket, stream)
            .map_err(|e| anyhow!("opening the page's DevTools socket: {e}"))?;
        ws.get_mut().set_read_timeout(Some(POLL))?;
        let (calls, pending) = mpsc::channel();
        let (events, received) = unbounded();
        std::thread::Builder::new()
            .name("cedian-cdp".into())
            .spawn(move || pump(ws, pending, events))?;
        Ok((
            Self {
                calls,
                next: Arc::new(AtomicU64::new(1)),
            },
            received,
        ))
    }

    /// Sends one call and returns its id, which its answer carries.
    pub fn call(&self, method: &str, params: Value) -> u64 {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let message = json!({ "id": id, "method": method, "params": params }).to_string();
        if self.calls.send(message).is_err() {
            log::debug!("browser: {method} after the connection ended");
        }
        id
    }
}

fn pump(
    mut ws: WebSocket<TcpStream>,
    calls: mpsc::Receiver<String>,
    events: UnboundedSender<CdpEvent>,
) {
    let why = 'connection: loop {
        loop {
            match calls.try_recv() {
                Ok(call) => {
                    if let Err(error) = ws.send(Message::text(call)) {
                        break 'connection error.to_string();
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                // The block let go of the page.
                Err(mpsc::TryRecvError::Disconnected) => {
                    let _ = ws.close(None);
                    return;
                }
            }
        }
        match ws.read() {
            Ok(Message::Text(text)) => {
                let Ok(value) = serde_json::from_str::<Value>(text.as_str()) else {
                    continue;
                };
                let event = if value["method"] == "Page.screencastFrame" {
                    let params = &value["params"];
                    let ack = json!({
                        "id": 0,
                        "method": "Page.screencastFrameAck",
                        "params": { "sessionId": params["sessionId"] },
                    });
                    if let Err(error) = ws.send(Message::text(ack.to_string())) {
                        break error.to_string();
                    }
                    match decode(params) {
                        Ok(frame) => frame,
                        Err(error) => {
                            log::warn!("browser frame: {error:#}");
                            continue;
                        }
                    }
                } else {
                    CdpEvent::Message(value)
                };
                if events.unbounded_send(event).is_err() {
                    let _ = ws.close(None);
                    return;
                }
            }
            Ok(Message::Close(_)) => break "the browser closed the page".to_owned(),
            Ok(_) => {}
            Err(tungstenite::Error::Io(e))
                if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(error) => break error.to_string(),
        }
    };
    let _ = events.unbounded_send(CdpEvent::Closed(why));
}

/// A frame's JPEG as the BGRA image GPUI draws, with the page size it shows.
fn decode(params: &Value) -> anyhow::Result<CdpEvent> {
    let data = params["data"]
        .as_str()
        .ok_or_else(|| anyhow!("a frame with no data"))?;
    let bytes = base64::engine::general_purpose::STANDARD.decode(data)?;
    let mut rgba =
        image::load_from_memory_with_format(&bytes, image::ImageFormat::Jpeg)?.into_rgba8();
    for pixel in rgba.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    let meta = &params["metadata"];
    Ok(CdpEvent::Frame {
        image: Arc::new(RenderImage::new(vec![image::Frame::new(rgba)])),
        width: meta["deviceWidth"].as_f64().unwrap_or(0.) as f32,
        height: meta["deviceHeight"].as_f64().unwrap_or(0.) as f32,
    })
}

#[cfg(test)]
mod tests {
    use std::{io::Read as _, io::Write as _, path::Path, time::Instant};

    use futures::StreamExt as _;

    use super::*;

    /// The first page's socket, from Chromium's `/json/list`.
    fn first_page(port: u16) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        write!(
            stream,
            "GET /json/list HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"
        )
        .expect("ask");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("timeout");
        // Chromium keeps the connection open: read until the timeout.
        let mut bytes = Vec::new();
        let _ = stream.read_to_end(&mut bytes);
        let reply = String::from_utf8_lossy(&bytes);
        let body = reply.split_once("\r\n\r\n").expect("body").1;
        let pages: Vec<Value> = serde_json::from_str(body).expect("json");
        pages
            .iter()
            .find(|p| p["type"] == "page")
            .and_then(|p| p["webSocketDebuggerUrl"].as_str())
            .expect("a page")
            .to_owned()
    }

    /// Against a real Chromium, when one is here (`CEDIAN_CHROMIUM`, or Playwright's).
    #[test]
    fn a_page_streams_decoded_frames_and_takes_calls() {
        let bin = std::env::var("CEDIAN_CHROMIUM").ok().or_else(|| {
            Path::new("/opt/pw-browsers/chromium")
                .exists()
                .then(|| "/opt/pw-browsers/chromium".to_owned())
        });
        let Some(bin) = bin else { return };
        let dir = tempfile::tempdir().expect("dir");
        let mut chromium = std::process::Command::new(bin)
            .args([
                "--headless=new",
                "--no-sandbox",
                "--remote-debugging-port=0",
                "about:blank",
            ])
            .arg(format!("--user-data-dir={}", dir.path().display()))
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("chromium");
        let port_file = dir.path().join("DevToolsActivePort");
        let since = Instant::now();
        while !port_file.exists() && since.elapsed() < Duration::from_secs(15) {
            std::thread::sleep(Duration::from_millis(100));
        }
        let port: u16 = std::fs::read_to_string(&port_file)
            .expect("port file")
            .lines()
            .next()
            .and_then(|l| l.parse().ok())
            .expect("port");
        let (cdp, mut events) = Cdp::connect(&first_page(port)).expect("connect");
        cdp.call("Page.enable", json!({}));
        cdp.call(
            "Emulation.setDeviceMetricsOverride",
            json!({ "width": 320, "height": 200, "deviceScaleFactor": 1, "mobile": false }),
        );
        cdp.call(
            "Page.navigate",
            json!({ "url": "data:text/html,<body style='background:red'>hi</body>" }),
        );
        cdp.call(
            "Page.startScreencast",
            json!({ "format": "jpeg", "maxWidth": 320, "maxHeight": 200 }),
        );
        let (mut frame, mut navigated) = (None, false);
        futures::executor::block_on(async {
            while let Some(event) = events.next().await {
                match event {
                    CdpEvent::Frame { width, height, .. } => frame = Some((width, height)),
                    CdpEvent::Message(m) if m["method"] == "Page.frameNavigated" => {
                        navigated = true
                    }
                    CdpEvent::Message(_) => {}
                    CdpEvent::Closed(why) => panic!("closed: {why}"),
                }
                if frame.is_some() && navigated {
                    break;
                }
            }
        });
        assert_eq!(frame, Some((320., 200.)));
        drop(cdp);
        let _ = chromium.kill();
        let _ = chromium.wait();
    }
}
