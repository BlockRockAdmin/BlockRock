//! Homeostatic monitoring for Zion Core.
//!
//! This module deliberately emits decisions only. It never scales anything
//! itself: collecting metrics must never create billable resources. Quando
//! `METABOLIC_WEBHOOK` e' configurato le decisioni vengono spedite a un
//! endpoint HTTP, e cosa farne — scalare, avvisare, aprire un ticket — resta
//! dall'altra parte. Il nodo non tiene credenziali cloud ne' tocca Docker.

use reqwest::Client;
use serde::Serialize;
use std::fs;
use std::io;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// The response the system would like an actuator to take.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MetabolicAction {
    Maintain,
    Hypertrophy,
    Atrophy,
}

/// Measurements supplied by a telemetry source.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct SystemVitals {
    /// CPU and memory are percentages in the inclusive range 0..=100.
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub latency_ms: f64,
    pub error_rate: f64,
}

impl SystemVitals {
    pub fn new(cpu_usage: f64, memory_usage: f64, latency_ms: f64, error_rate: f64) -> Self {
        Self {
            cpu_usage: cpu_usage.clamp(0.0, 100.0),
            memory_usage: memory_usage.clamp(0.0, 100.0),
            latency_ms: latency_ms.max(0.0),
            error_rate: error_rate.max(0.0),
        }
    }
}

/// Tunable, side-effect-free policy for the hypothalamus.
#[derive(Debug, Clone)]
pub struct HypothalamusConfig {
    pub stress_threshold: f64,
    pub relaxation_threshold: f64,
    pub latency_threshold_ms: f64,
    pub error_rate_weight: f64,
    pub sustained_stress_cycles: u32,
    pub sustained_relaxation_cycles: u32,
}

impl Default for HypothalamusConfig {
    fn default() -> Self {
        Self {
            stress_threshold: 85.0,
            relaxation_threshold: 20.0,
            latency_threshold_ms: 100.0,
            error_rate_weight: 10.0,
            sustained_stress_cycles: 5,
            // Scaling down also needs confirmation: a quiet instant must not
            // remove capacity that a workload needs moments later.
            sustained_relaxation_cycles: 5,
        }
    }
}

/// A published snapshot, suitable for the REST dashboard and alerting.
#[derive(Debug, Clone, Serialize)]
pub struct MetabolicStatus {
    pub action: MetabolicAction,
    pub stress_score: f64,
    pub stress_cycles: u32,
    pub relaxation_cycles: u32,
    pub vitals: SystemVitals,
}

impl Default for MetabolicStatus {
    fn default() -> Self {
        Self {
            action: MetabolicAction::Maintain,
            stress_score: 0.0,
            stress_cycles: 0,
            relaxation_cycles: 0,
            vitals: SystemVitals::new(0.0, 0.0, 0.0, 0.0),
        }
    }
}

pub type SharedMetabolicStatus = Arc<RwLock<MetabolicStatus>>;

pub fn shared_metabolic_status() -> SharedMetabolicStatus {
    Arc::new(RwLock::new(MetabolicStatus::default()))
}

/// Stateful policy engine. It has no network, Docker, or cloud credentials.
pub struct Hypothalamus {
    config: HypothalamusConfig,
    stress_cycles: u32,
    relaxation_cycles: u32,
}

impl Default for Hypothalamus {
    fn default() -> Self {
        Self::new(HypothalamusConfig::default())
    }
}

impl Hypothalamus {
    pub fn new(config: HypothalamusConfig) -> Self {
        Self {
            config,
            stress_cycles: 0,
            relaxation_cycles: 0,
        }
    }

    /// A weighted score: CPU and memory are primary; latency and errors add
    /// pressure because they are often more consequential than raw utilisation.
    pub fn calculate_system_stress(&self, vitals: SystemVitals) -> f64 {
        let latency_penalty = if vitals.latency_ms > self.config.latency_threshold_ms {
            20.0
        } else {
            0.0
        };
        (vitals.cpu_usage * 0.4
            + vitals.memory_usage * 0.4
            + latency_penalty
            + vitals.error_rate * self.config.error_rate_weight)
            .min(100.0)
    }

    /// Evaluate one telemetry sample. A requested action is emitted only after
    /// the corresponding condition persists for the configured number of cycles.
    pub fn evaluate(&mut self, vitals: SystemVitals) -> MetabolicStatus {
        let stress_score = self.calculate_system_stress(vitals);
        let action = if stress_score >= self.config.stress_threshold {
            self.stress_cycles = self.stress_cycles.saturating_add(1);
            self.relaxation_cycles = 0;
            if self.stress_cycles >= self.config.sustained_stress_cycles.max(1) {
                self.stress_cycles = 0;
                MetabolicAction::Hypertrophy
            } else {
                MetabolicAction::Maintain
            }
        } else if stress_score <= self.config.relaxation_threshold {
            self.relaxation_cycles = self.relaxation_cycles.saturating_add(1);
            self.stress_cycles = 0;
            if self.relaxation_cycles >= self.config.sustained_relaxation_cycles.max(1) {
                self.relaxation_cycles = 0;
                MetabolicAction::Atrophy
            } else {
                MetabolicAction::Maintain
            }
        } else {
            self.stress_cycles = 0;
            self.relaxation_cycles = 0;
            MetabolicAction::Maintain
        };

        MetabolicStatus {
            action,
            stress_score,
            stress_cycles: self.stress_cycles,
            relaxation_cycles: self.relaxation_cycles,
            vitals,
        }
    }
}

/// A synchronous source makes it straightforward to use Prometheus, IoT, or a
/// deterministic test source without coupling the policy to a transport.
pub trait VitalsSource: Send + 'static {
    fn collect(&mut self) -> io::Result<SystemVitals>;
}

/// Minimal Linux source for a node running Zion Core. Latency and error rate are
/// intentionally zero here: adapters may supply application-specific values.
pub struct ProcfsVitalsSource {
    previous_cpu: Option<(u64, u64)>,
}

impl Default for ProcfsVitalsSource {
    fn default() -> Self {
        Self { previous_cpu: None }
    }
}

impl VitalsSource for ProcfsVitalsSource {
    fn collect(&mut self) -> io::Result<SystemVitals> {
        let (total, idle) = read_cpu_totals()?;
        let cpu_usage = self
            .previous_cpu
            .replace((total, idle))
            .and_then(|(previous_total, previous_idle)| {
                let total_delta = total.checked_sub(previous_total)?;
                let idle_delta = idle.checked_sub(previous_idle)?;
                (total_delta > 0).then(|| {
                    100.0 * (total_delta.saturating_sub(idle_delta) as f64 / total_delta as f64)
                })
            })
            .unwrap_or(0.0);
        let memory_usage = read_memory_usage()?;
        Ok(SystemVitals::new(cpu_usage, memory_usage, 0.0, 0.0))
    }
}

fn read_cpu_totals() -> io::Result<(u64, u64)> {
    let stat = fs::read_to_string("/proc/stat")?;
    let fields: Vec<u64> = stat
        .lines()
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .skip(1)
        .map(str::parse)
        .collect::<Result<_, _>>()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if fields.len() < 4 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid /proc/stat"));
    }
    let total = fields.iter().sum();
    let idle = fields[3] + fields.get(4).copied().unwrap_or(0);
    Ok((total, idle))
}

fn read_memory_usage() -> io::Result<f64> {
    let meminfo = fs::read_to_string("/proc/meminfo")?;
    let mut total = None;
    let mut available = None;
    for line in meminfo.lines() {
        let mut parts = line.split_whitespace();
        match (parts.next(), parts.next()) {
            (Some("MemTotal:"), Some(value)) => total = value.parse::<f64>().ok(),
            (Some("MemAvailable:"), Some(value)) => available = value.parse::<f64>().ok(),
            _ => {}
        }
    }
    match (total, available) {
        (Some(total), Some(available)) if total > 0.0 => Ok(100.0 * (1.0 - available / total)),
        _ => Err(io::Error::new(io::ErrorKind::InvalidData, "invalid /proc/meminfo")),
    }
}

/// Un endpoint lento non deve trattenere il loop di monitoraggio.
const WEBHOOK_TIMEOUT: Duration = Duration::from_secs(5);

/// Spedisce le decisioni metaboliche a un endpoint HTTP.
///
/// Il nodo continua a non azionare niente: manda la decisione e basta. Chi la
/// riceve decide se scalare, avvisare o ignorarla, e il nodo non ha bisogno di
/// credenziali cloud o dell'accesso al socket Docker per questo.
pub struct MetabolicWebhook {
    client: Client,
    url: String,
}

impl MetabolicWebhook {
    pub fn new(url: String) -> Self {
        let client = Client::builder()
            .timeout(WEBHOOK_TIMEOUT)
            .build()
            .unwrap_or_default();
        Self { client, url }
    }

    /// Notifica la decisione. Non restituisce errore di proposito: un webhook
    /// irraggiungibile e' un problema di chi ascolta, e non deve fermare il
    /// monitoraggio del nodo.
    async fn notify(&self, status: &MetabolicStatus) {
        match self.client.post(&self.url).json(status).send().await {
            Ok(response) if response.status().is_success() => tracing::info!(
                url = %self.url,
                action = ?status.action,
                "decisione metabolica notificata"
            ),
            Ok(response) => tracing::warn!(
                url = %self.url,
                status = %response.status(),
                "il webhook metabolico ha rifiutato la decisione"
            ),
            Err(error) => tracing::warn!(
                url = %self.url,
                %error,
                "webhook metabolico non raggiungibile"
            ),
        }
    }
}

/// Lascia passare solo i cambi di decisione: a un tick ogni 15 secondi,
/// notificare ogni volta `Maintain` sarebbe rumore, e il segnale che conta e'
/// il momento in cui la decisione cambia.
#[derive(Default)]
struct ChangeGate {
    last: Option<MetabolicAction>,
}

impl ChangeGate {
    fn should_notify(&mut self, action: MetabolicAction) -> bool {
        let changed = self.last != Some(action);
        self.last = Some(action);
        changed
    }
}

/// Run the autonomous monitoring loop and publish the latest decision. The
/// caller owns the task handle and can abort it during graceful shutdown.
pub async fn run_monitoring_loop<S>(
    mut source: S,
    mut hypothalamus: Hypothalamus,
    status: SharedMetabolicStatus,
    interval: Duration,
    webhook: Option<MetabolicWebhook>,
) where
    S: VitalsSource,
{
    let listener = if webhook.is_some() {
        "notifico il webhook"
    } else {
        "nessun attuatore configurato"
    };
    let mut gate = ChangeGate::default();

    loop {
        match source.collect() {
            Ok(vitals) => {
                let next = hypothalamus.evaluate(vitals);
                match next.action {
                    MetabolicAction::Hypertrophy => tracing::warn!(
                        stress_score = next.stress_score,
                        "l'ipotalamo chiede piu' capacita'; {}",
                        listener
                    ),
                    MetabolicAction::Atrophy => tracing::info!(
                        stress_score = next.stress_score,
                        "l'ipotalamo chiede meno capacita'; {}",
                        listener
                    ),
                    MetabolicAction::Maintain => tracing::debug!(
                        stress_score = next.stress_score,
                        "l'ipotalamo mantiene la capacita' attuale"
                    ),
                }

                if let Some(webhook) = &webhook {
                    if gate.should_notify(next.action) {
                        webhook.notify(&next).await;
                    }
                }

                *status.write().await = next;
            }
            Err(error) => tracing::warn!(%error, "unable to collect metabolic telemetry"),
        }
        tokio::time::sleep(interval).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gate_lets_through_only_changes() {
        let mut gate = ChangeGate::default();

        // La prima decisione e' sempre una notizia.
        assert!(gate.should_notify(MetabolicAction::Maintain));
        // Ripeterla non lo e'.
        assert!(!gate.should_notify(MetabolicAction::Maintain));
        assert!(!gate.should_notify(MetabolicAction::Maintain));

        assert!(gate.should_notify(MetabolicAction::Hypertrophy));
        assert!(!gate.should_notify(MetabolicAction::Hypertrophy));

        // Anche il rientro alla normalita' va notificato: dice "rientra".
        assert!(gate.should_notify(MetabolicAction::Maintain));
    }

    fn policy() -> HypothalamusConfig {
        HypothalamusConfig {
            sustained_stress_cycles: 3,
            sustained_relaxation_cycles: 2,
            ..HypothalamusConfig::default()
        }
    }

    #[test]
    fn sustained_stress_requests_hypertrophy() {
        let mut brain = Hypothalamus::new(policy());
        let stressed = SystemVitals::new(100.0, 100.0, 150.0, 0.0);
        assert_eq!(brain.evaluate(stressed).action, MetabolicAction::Maintain);
        assert_eq!(brain.evaluate(stressed).action, MetabolicAction::Maintain);
        assert_eq!(brain.evaluate(stressed).action, MetabolicAction::Hypertrophy);
    }

    #[test]
    fn transient_stress_does_not_request_scaling() {
        let mut brain = Hypothalamus::new(policy());
        let stressed = SystemVitals::new(100.0, 100.0, 0.0, 0.0);
        let normal = SystemVitals::new(50.0, 50.0, 0.0, 0.0);
        brain.evaluate(stressed);
        brain.evaluate(stressed);
        assert_eq!(brain.evaluate(normal).action, MetabolicAction::Maintain);
        assert_eq!(brain.evaluate(stressed).action, MetabolicAction::Maintain);
    }

    #[test]
    fn sustained_underuse_requests_atrophy() {
        let mut brain = Hypothalamus::new(policy());
        let quiet = SystemVitals::new(0.0, 0.0, 0.0, 0.0);
        assert_eq!(brain.evaluate(quiet).action, MetabolicAction::Maintain);
        assert_eq!(brain.evaluate(quiet).action, MetabolicAction::Atrophy);
    }
}
