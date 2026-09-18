//! Verifica che le decisioni metaboliche escano davvero dal nodo, contro un
//! server HTTP vero invece che contro un mock.

use std::io;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use zion_core::monitoring::{
    run_monitoring_loop, shared_metabolic_status, Hypothalamus, HypothalamusConfig,
    MetabolicWebhook, SystemVitals, VitalsSource,
};

/// Sorgente che riporta sempre gli stessi valori, così la decisione è
/// prevedibile.
struct FixedVitals(SystemVitals);

impl VitalsSource for FixedVitals {
    fn collect(&mut self) -> io::Result<SystemVitals> {
        Ok(self.0)
    }
}

/// Un server che accetta una richiesta, ne restituisce il corpo e chiude.
async fn capture_one_request(listener: TcpListener, seen: Arc<Mutex<Vec<String>>>) {
    while let Ok((mut socket, _)) = listener.accept().await {
        let mut buffer = vec![0u8; 4096];
        if let Ok(n) = socket.read(&mut buffer).await {
            seen.lock()
                .await
                .push(String::from_utf8_lossy(&buffer[..n]).to_string());
        }
        let _ = socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
            .await;
        let _ = socket.shutdown().await;
    }
}

#[tokio::test]
async fn a_capacity_request_reaches_the_webhook() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/metabolism", listener.local_addr().unwrap());

    let seen = Arc::new(Mutex::new(Vec::new()));
    tokio::spawn(capture_one_request(listener, Arc::clone(&seen)));

    // Un nodo sotto stress costante: dopo pochi cicli l'ipotalamo chiede
    // capacità.
    let stressed = SystemVitals::new(99.0, 99.0, 500.0, 0.5);
    let policy = HypothalamusConfig {
        sustained_stress_cycles: 1,
        ..HypothalamusConfig::default()
    };

    let loop_handle = tokio::spawn(run_monitoring_loop(
        FixedVitals(stressed),
        Hypothalamus::new(policy),
        shared_metabolic_status(),
        Duration::from_millis(10),
        Some(MetabolicWebhook::new(url)),
    ));

    // Attende che qualcosa arrivi, senza inchiodare il test se non arriva.
    let mut body = String::new();
    for _ in 0..200 {
        if let Some(request) = seen.lock().await.first() {
            body = request.clone();
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    loop_handle.abort();

    assert!(!body.is_empty(), "il webhook non ha ricevuto nulla");
    assert!(body.starts_with("POST /metabolism"), "richiesta: {}", body);
    assert!(
        body.contains("hypertrophy"),
        "il corpo deve portare la decisione: {}",
        body
    );
    assert!(body.contains("stress_score"), "corpo: {}", body);

    // E non deve ripetersi a ogni tick: la decisione non cambia piu'.
    let before = seen.lock().await.len();
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(
        before,
        seen.lock().await.len(),
        "una decisione invariata non va rinotificata"
    );
}
