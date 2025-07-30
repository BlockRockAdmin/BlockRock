// src/network/mod.rs
pub mod p2p;

// Esporta la funzione e il tipo di evento P2P aggiornati
pub use p2p::{start_p2p_node, CustomEvent as MyBehaviourEvent}; // CustomEvent è l'OutEvent del nostro comportamento
