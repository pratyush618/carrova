//! A loopback UDP relay that can be cut, to make a peer vanish without a
//! close (as a dropped Wi-Fi link does).

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Forwards datagrams between one client and `target` until cut.
pub struct Relay {
    addr: SocketAddrV4,
    cut: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
}

impl Relay {
    /// Relay from a fresh loopback port (see [`Self::addr`]) to `target`.
    pub fn start(target: SocketAddrV4) -> Self {
        let front = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let back = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        back.connect(target).unwrap();
        for socket in [&front, &back] {
            socket
                .set_read_timeout(Some(Duration::from_millis(20)))
                .unwrap();
        }
        let SocketAddr::V4(addr) = front.local_addr().unwrap() else {
            panic!("loopback relay is IPv4");
        };
        let relay = Self {
            addr,
            cut: Arc::default(),
            stop: Arc::default(),
        };
        let client = Arc::new(Mutex::new(None::<SocketAddr>));
        relay.pump({
            let (front, back, client) = (
                front.try_clone().unwrap(),
                back.try_clone().unwrap(),
                Arc::clone(&client),
            );
            move |buf: &mut [u8], open: bool| {
                if let Ok((n, from)) = front.recv_from(buf) {
                    *client.lock().unwrap() = Some(from);
                    if open {
                        let _ = back.send(&buf[..n]);
                    }
                }
            }
        });
        relay.pump(move |buf: &mut [u8], open: bool| {
            if let Ok(n) = back.recv(buf) {
                let to = *client.lock().unwrap();
                if let (true, Some(to)) = (open, to) {
                    let _ = front.send_to(&buf[..n], to);
                }
            }
        });
        relay
    }

    /// Run `step(buffer, open)` on a thread until the relay is dropped.
    fn pump(&self, mut step: impl FnMut(&mut [u8], bool) + Send + 'static) {
        let (cut, stop) = (Arc::clone(&self.cut), Arc::clone(&self.stop));
        thread::spawn(move || {
            let mut buf = vec![0u8; 65_536];
            while !stop.load(Ordering::Acquire) {
                step(&mut buf, !cut.load(Ordering::Acquire));
            }
        });
    }

    /// Where clients send.
    pub fn addr(&self) -> SocketAddrV4 {
        self.addr
    }

    /// Drop every datagram from now on, both ways.
    pub fn cut(&self) {
        self.cut.store(true, Ordering::Release);
    }
}

impl Drop for Relay {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}
