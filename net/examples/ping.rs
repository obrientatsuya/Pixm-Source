/// Teste da Fase 1 — ping/pong entre dois processos.
///
/// Uso:
///   Terminal 1:  cargo run --example ping -- server
///   Terminal 2:  cargo run --example ping -- client
///
/// Resultado esperado: 1000 pacotes trocados, mede RTT médio e loss.

use net::transport::UdpTransport;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

const SERVER_ADDR: &str = "127.0.0.1:7000";
const PING_COUNT:  usize = 1000;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("server") => run_server(),
        Some("client") => run_client(),
        _ => eprintln!("uso: ping [server|client]"),
    }
}

fn run_server() {
    let addr: SocketAddr = SERVER_ADDR.parse().unwrap();
    let mut transport = UdpTransport::bind(addr).unwrap();
    println!("[server] ouvindo em {}", SERVER_ADDR);

    let mut count = 0usize;
    loop {
        let packets = transport.poll();
        for pkt in packets {
            // Devolve o payload (pong)
            transport.send(pkt.from, &pkt.payload, false).unwrap();
            count += 1;
            if count % 100 == 0 {
                println!("[server] {count} pongs enviados");
            }
            if count >= PING_COUNT { return; }
        }
        transport.flush_reliable().unwrap();
        std::thread::sleep(Duration::from_micros(100));
    }
}

fn run_client() {
    let local: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let server: SocketAddr = SERVER_ADDR.parse().unwrap();
    let mut transport = UdpTransport::bind(local).unwrap();
    println!("[client] conectando a {SERVER_ADDR}");

    std::thread::sleep(Duration::from_millis(100)); // aguarda server subir

    let mut sent      = 0usize;
    let mut received  = 0usize;
    let mut rtt_sum   = Duration::ZERO;

    while sent < PING_COUNT {
        // Envia ping com timestamp embutido no payload
        let ts = Instant::now();
        let ts_nanos = ts.elapsed().as_nanos() as u64; // relativo — só pra medir RTT
        let payload = ts_nanos.to_le_bytes();
        transport.send(server, &payload, false).unwrap();
        sent += 1;

        // Aguarda pong (até 100ms)
        let deadline = Instant::now() + Duration::from_millis(100);
        loop {
            let packets = transport.poll();
            for pkt in packets {
                let rtt = Instant::now().duration_since(ts);
                rtt_sum += rtt;
                received += 1;
                let _ = pkt;
            }
            if received >= sent || Instant::now() > deadline { break; }
            std::thread::sleep(Duration::from_micros(100));
        }

        transport.flush_reliable().unwrap();
    }

    let loss_pct = 100.0 * (sent - received) as f64 / sent as f64;
    let avg_rtt  = if received > 0 { rtt_sum / received as u32 } else { Duration::ZERO };

    println!("─────────────────────────────");
    println!("Enviados:   {sent}");
    println!("Recebidos:  {received}");
    println!("Perda:      {loss_pct:.1}%");
    println!("RTT médio:  {:.3}ms", avg_rtt.as_secs_f64() * 1000.0);
    println!("─────────────────────────────");
}
