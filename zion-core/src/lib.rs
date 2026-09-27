// src/lib.rs
pub mod api;
pub mod config;
pub mod identity;
pub mod ledger;
pub mod monitoring;
pub mod network;
pub mod server;
pub mod storage;

// Esportazioni dei moduli API (se esistono e contengono le funzioni indicate)
pub use api::grpc;
pub use api::prometheus; // Modulo prometheus.rs con funzioni di metriche
pub use api::rest; // Modulo rest.rs con funzioni HTTP // Modulo grpc.rs con funzioni gRPC

// Esportazioni dei moduli di rete
pub use network::p2p::{start_p2p_node, CustomEvent}; // Usa CustomEvent invece di MyBehaviourEvent

// Esportazione della configurazione
pub use config::Config; // La tua struct Config
