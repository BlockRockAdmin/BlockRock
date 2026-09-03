// src/lib.rs
pub mod api;
pub mod network;
pub mod config;
pub mod monitoring;

// Esportazioni dei moduli API (se esistono e contengono le funzioni indicate)
pub use api::rest; // Modulo rest.rs con funzioni HTTP
pub use api::prometheus; // Modulo prometheus.rs con funzioni di metriche
pub use api::grpc; // Modulo grpc.rs con funzioni gRPC

// Esportazioni dei moduli di rete
pub use network::p2p::{start_p2p_node, CustomEvent}; // Usa CustomEvent invece di MyBehaviourEvent

// Esportazione della configurazione
pub use config::Config; // La tua struct Config
