//! La strada per cui una transazione entra in catena.
//!
//! REST e gRPC sono due porte sullo stesso nodo: se ognuna avesse la propria
//! copia di questa sequenza, prima o poi divergerebbero — e la regola che
//! protegge il conto di conio finirebbe su una porta sola.

use blockrock_core::{
    blockchain::{Blockchain, MINT_ACCOUNT},
    transaction::Transaction,
};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::api::rest::BlockAnnouncer;
use crate::identity::NodeIdentity;
use crate::network::sync::announcement_of;
use crate::storage::ChainStore;

/// Transazione accettata: il suo id e quante ne restano in attesa.
pub struct Queued {
    pub id: String,
    pub pending: usize,
}

/// Distingue una richiesta malfatta da un guasto del nodo, così ogni
/// interfaccia la traduce nel proprio codice d'errore senza indovinare.
#[derive(Debug)]
pub enum SubmitError {
    /// Il chiamante ha sbagliato: firma, nonce, fondi, duplicato.
    Rejected(String),
    /// Il nodo non ce l'ha fatta: per esempio un blocco sigillato ma non
    /// persistito, che non e' colpa di chi ha inviato.
    Internal(String),
}

impl std::fmt::Display for SubmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubmitError::Rejected(message) | SubmitError::Internal(message) => {
                write!(f, "{}", message)
            }
        }
    }
}

/// Accoda la transazione e, se il mempool viene sigillato, persiste il blocco e
/// lo annuncia ai peer.
pub async fn submit(
    transaction: Transaction,
    blockchain: &Arc<Mutex<Blockchain>>,
    identity: &NodeIdentity,
    store: &ChainStore,
    announcer: &BlockAnnouncer,
) -> Result<Queued, SubmitError> {
    // Il conio non passa da nessuna interfaccia: e' una regola del nodo, non
    // della singola porta.
    if transaction.sender == MINT_ACCOUNT {
        return Err(SubmitError::Rejected(format!(
            "'{}' mints value and cannot be used from the API",
            MINT_ACCOUNT
        )));
    }

    let id = transaction.id.clone();
    let mut chain = blockchain.lock().await;
    chain
        .queue_transaction(transaction)
        .map_err(|e| SubmitError::Rejected(e.to_string()))?;

    let pending = chain.pending_count();

    // Si sigilla subito solo quando il lotto e' gia' pieno: cosi' una raffica
    // non aspetta il tick, ma il traffico normale si accumula e finisce in
    // blocchi con piu' di una transazione dentro. Prima si sigillava a ogni
    // invio, quindi ogni transazione otteneva un blocco tutto suo e il tick
    // da 5 secondi trovava sempre il mempool vuoto.
    let worth_sealing = chain.is_worth_sealing();
    drop(chain);

    if worth_sealing {
        seal_pending(blockchain, identity, store, announcer).await?;
    }

    Ok(Queued { id, pending })
}

/// Sigilla tutto cio' che e' in attesa, persiste il blocco e lo annuncia.
///
/// E' quello che fa il tick periodico, e vive qui per due motivi: il tick e
/// l'invio condividono la stessa sequenza di persistenza e annuncio, e finche'
/// era inline dentro `node.rs` nessun test poteva chiudere un blocco senza
/// reimplementarla.
///
/// Restituisce l'indice del blocco, o `None` se non c'era niente da sigillare.
pub async fn seal_pending(
    blockchain: &Arc<Mutex<Blockchain>>,
    identity: &NodeIdentity,
    store: &ChainStore,
    announcer: &BlockAnnouncer,
) -> Result<Option<u32>, SubmitError> {
    let mut chain = blockchain.lock().await;
    let sealed = chain
        .seal_mempool(&identity.name, &identity.signing_key)
        .map_err(|e| SubmitError::Internal(e.to_string()))?;

    let Some(index) = sealed else {
        return Ok(None);
    };

    store
        .save(&chain)
        .map_err(|error| SubmitError::Internal(format!("block sealed but not persisted: {}", error)))?;

    if let Some(block) = chain.blocks.last() {
        let announcement = announcement_of(&chain, block.clone());
        drop(chain);
        let _ = announcer.try_send(announcement);
    }

    Ok(Some(index))
}
