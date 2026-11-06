//! Receives a watched screen's frames from the voice engine over a private local socket.
use std::{
    io::{ErrorKind, Read},
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

/// Far larger than the engine's 1280-wide frames, so a bad header can't allocate wildly.
const LARGEST_SIDE: u32 = 4096;

pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// In the BGRA order GPUI paints.
    pub pixels: Vec<u8>,
}

pub struct FrameSink {
    pub port: u16,
    pub token: String,
    latest: Arc<Mutex<Option<Frame>>>,
    stop: Arc<AtomicBool>,
}

impl Drop for FrameSink {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

impl FrameSink {
    pub fn start() -> Result<Self, String> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|error| error.to_string())?;
        let port = listener.local_addr().map_err(|error| error.to_string())?.port();
        listener.set_nonblocking(true).map_err(|error| error.to_string())?;
        let token = uuid::Uuid::new_v4().simple().to_string();
        let latest = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let (thread_token, thread_latest, thread_stop) = (token.clone(), latest.clone(), stop.clone());
        std::thread::spawn(move || run(listener, thread_token, thread_latest, thread_stop));
        Ok(Self { port, token, latest, stop })
    }

    /// The newest frame since the last call. Older ones are skipped.
    pub fn take(&self) -> Option<Frame> {
        self.latest.lock().ok()?.take()
    }
}

fn run(listener: TcpListener, token: String, latest: Arc<Mutex<Option<Frame>>>, stop: Arc<AtomicBool>) {
    while !stop.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((mut socket, _)) => {
                let _ = socket.set_nonblocking(false);
                let _ = socket.set_read_timeout(Some(Duration::from_millis(250)));
                let mut offered = vec![0; token.len()];
                // Anything else on this machine could connect, so it must know the token.
                if read_exact(&mut socket, &mut offered, &stop).is_ok() && offered == token.as_bytes() {
                    let _ = receive(&mut socket, &latest, &stop);
                    return;
                }
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => return,
        }
    }
}

fn receive(
    socket: &mut TcpStream,
    latest: &Mutex<Option<Frame>>,
    stop: &AtomicBool,
) -> std::io::Result<()> {
    let mut header = [0; 8];
    loop {
        read_exact(socket, &mut header, stop)?;
        let width = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
        let height = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
        if width == 0 || height == 0 || width > LARGEST_SIDE || height > LARGEST_SIDE {
            return Err(ErrorKind::InvalidData.into());
        }
        let mut pixels = vec![0; (width * height * 4) as usize];
        read_exact(socket, &mut pixels, stop)?;
        if let Ok(mut slot) = latest.lock() {
            *slot = Some(Frame { width, height, pixels });
        }
    }
}

/// Like `read_exact`, but keeps a partial read through timeouts and notices a stop.
fn read_exact(socket: &mut TcpStream, buffer: &mut [u8], stop: &AtomicBool) -> std::io::Result<()> {
    let mut filled = 0;
    while filled < buffer.len() {
        if stop.load(Ordering::Acquire) {
            return Err(ErrorKind::Interrupted.into());
        }
        match socket.read(&mut buffer[filled..]) {
            Ok(0) => return Err(ErrorKind::UnexpectedEof.into()),
            Ok(count) => filled += count,
            Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn wait_for(sink: &FrameSink) -> Option<Frame> {
        for _ in 0..100 {
            if let Some(frame) = sink.take() {
                return Some(frame);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        None
    }

    #[test]
    fn only_the_engine_holding_the_token_can_deliver_frames() {
        let sink = FrameSink::start().unwrap();
        let mut stranger = TcpStream::connect(("127.0.0.1", sink.port)).unwrap();
        stranger.write_all(&vec![b'x'; sink.token.len()]).unwrap();
        let _ = stranger.write_all(&[2, 0, 0, 0, 1, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8]);
        std::thread::sleep(Duration::from_millis(150));
        assert!(sink.take().is_none());

        let mut engine = TcpStream::connect(("127.0.0.1", sink.port)).unwrap();
        engine.write_all(sink.token.as_bytes()).unwrap();
        engine.write_all(&[2, 0, 0, 0, 1, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8]).unwrap();
        let frame = wait_for(&sink).unwrap();
        assert_eq!((frame.width, frame.height), (2, 1));
        assert_eq!(frame.pixels, [1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn an_absurd_frame_size_is_refused() {
        let sink = FrameSink::start().unwrap();
        let mut engine = TcpStream::connect(("127.0.0.1", sink.port)).unwrap();
        engine.write_all(sink.token.as_bytes()).unwrap();
        engine.write_all(&[255, 255, 255, 255, 255, 255, 255, 255]).unwrap();
        std::thread::sleep(Duration::from_millis(100));
        assert!(sink.take().is_none());
    }
}
