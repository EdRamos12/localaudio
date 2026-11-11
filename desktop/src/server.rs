use crate::protocol::{TCP_PORT, UDP_PORT};
use anyhow::{anyhow, Context, Result};
use mdns_sd::{ServiceDaemon, ServiceInfo};
use std::io::{BufRead, BufReader, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

struct Client {
    udp: SocketAddr,
    tx: Sender<String>,
}

pub struct Server {
    clients: Mutex<Vec<Client>>,
    udp: UdpSocket,
    passphrase: Arc<Mutex<String>>,
    pub name: String,
    pub address: Ipv4Addr,
    volume: AtomicU8,
    _mdns: ServiceDaemon,
}

impl Server {
    pub fn start(name: String, passphrase: Arc<Mutex<String>>, lan_ip: Ipv4Addr) -> Result<Arc<Self>> {
        let udp = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, UDP_PORT))
            .with_context(|| format!("bind udp {UDP_PORT}"))?;
        let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, TCP_PORT))
            .with_context(|| format!("bind tcp {TCP_PORT}"))?;
        let mdns = ServiceDaemon::new().context("start mDNS")?;
        let host = format!("{}.local.", sanitize(&name));
        let tcp_port = TCP_PORT.to_string();
        let properties = [("tcp", tcp_port.as_str()), ("name", name.as_str())];
        let service = ServiceInfo::new(
            "_localaudio._udp.local.",
            &name,
            &host,
            IpAddr::V4(lan_ip),
            UDP_PORT,
            &properties[..],
        )
        .map_err(|err| anyhow!("mDNS service: {err}"))?;
        mdns.register(service)
            .map_err(|err| anyhow!("mDNS register: {err}"))?;

        let server = Arc::new(Self {
            clients: Mutex::new(Vec::new()),
            udp,
            passphrase,
            name,
            address: lan_ip,
            volume: AtomicU8::new(100),
            _mdns: mdns,
        });
        let accept_server = Arc::clone(&server);
        thread::Builder::new()
            .name("localaudio-tcp".into())
            .spawn(move || accept_loop(listener, accept_server))
            .context("spawn tcp thread")?;
        eprintln!("listening on {lan_ip} tcp {TCP_PORT} udp {UDP_PORT}");
        Ok(server)
    }

    pub fn send_audio(&self, packet: &[u8]) {
        let targets: Vec<SocketAddr> = self
            .clients
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .iter()
            .map(|client| client.udp)
            .collect();
        for target in targets {
            if let Err(err) = self.udp.send_to(packet, target) {
                eprintln!("udp send to {target} failed: {err}");
            }
        }
    }

    pub fn listener_count(&self) -> usize {
        self.clients.lock().unwrap_or_else(|err| err.into_inner()).len()
    }

    pub fn volume(&self) -> u8 {
        self.volume.load(Ordering::Relaxed)
    }

    pub fn set_volume(&self, volume: u8) {
        self.volume.store(volume.min(100), Ordering::Relaxed);
        let line = format!("VOLUME {}", volume.min(100));
        let clients = self.clients.lock().unwrap_or_else(|err| err.into_inner());
        for client in clients.iter() {
            let _ = client.tx.send(line.clone());
        }
    }

    fn remove_udp(&self, addr: SocketAddr) {
        self.clients
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .retain(|client| client.udp != addr);
    }
}

fn accept_loop(listener: TcpListener, server: Arc<Server>) {
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let server = Arc::clone(&server);
                thread::spawn(move || {
                    if let Err(err) = handle_client(stream, &server) {
                        eprintln!("client ended: {err}");
                    }
                });
            }
            Err(err) => eprintln!("accept failed: {err}"),
        }
    }
}

fn handle_client(stream: TcpStream, server: &Server) -> Result<()> {
    let peer = stream.peer_addr().context("peer address")?;
    eprintln!("control connection from {peer}");
    stream.set_nodelay(true).ok();
    stream.set_read_timeout(Some(Duration::from_secs(20))).ok();
    let mut writer = stream.try_clone().context("clone tcp")?;
    let (tx, rx) = mpsc::channel::<String>();
    thread::spawn(move || {
        while let Ok(line) = rx.recv() {
            if writeln!(writer, "{line}").is_err() {
                break;
            }
            writer.flush().ok();
        }
    });

    let reader = BufReader::new(stream);
    let mut welcomed = false;
    let mut udp_addr: Option<SocketAddr> = None;
    for line in reader.lines() {
        let Ok(line) = line else { break };
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("HELLO") if !welcomed => {
                let _client_name = parts.next();
                let token = parts.next().unwrap_or("-");
                let pass = server
                    .passphrase
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .clone();
                if !pass.is_empty() && token != pass {
                    let _ = tx.send("DENY".to_string());
                    break;
                }
                let _ = tx.send(format!("WELCOME {} 48000 2 opus 20", server.name));
                let _ = tx.send(format!("VOLUME {}", server.volume()));
                welcomed = true;
            }
            Some("LISTEN") if welcomed => {
                let Some(port) = parts.next().and_then(|value| value.parse::<u16>().ok()) else {
                    continue;
                };
                if let Some(previous) = udp_addr.take() {
                    server.remove_udp(previous);
                }
                let addr = SocketAddr::new(peer.ip(), port);
                udp_addr = Some(addr);
                server
                    .clients
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .push(Client {
                        udp: addr,
                        tx: tx.clone(),
                    });
                eprintln!("sending audio to {addr}");
            }
            Some("STOP") => {
                if let Some(addr) = udp_addr.take() {
                    server.remove_udp(addr);
                    eprintln!("stopped audio to {addr}");
                }
            }
            Some("PING") => {
                let _ = tx.send("PONG".to_string());
            }
            _ => {}
        }
    }
    if let Some(addr) = udp_addr {
        server.remove_udp(addr);
    }
    Ok(())
}

fn sanitize(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect();
    if cleaned.is_empty() {
        "localaudio".to_string()
    } else {
        cleaned
    }
}

pub fn lan_ip() -> Result<Ipv4Addr> {
    let output = std::process::Command::new("ip")
        .args(["-4", "route", "get", "1.1.1.1"])
        .output()
        .context("ip route")?;
    let text = String::from_utf8_lossy(&output.stdout);
    let ip = text
        .split_whitespace()
        .skip_while(|part| *part != "src")
        .nth(1)
        .ok_or_else(|| anyhow!("no src address in `{text}`"))?;
    ip.parse().context("parse lan address")
}
