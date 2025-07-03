# Zion-Core: Orchestratore di BlockRock Hyperion

![Build Status](https://github.com/BlockRockAdmin/BlockRock/actions/workflows/ci.yml/badge.svg)
![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Rust](https://img.shields.io/badge/Rust-nightly-orange.svg)

**Zion-Core** è l'orchestratore del progetto **BlockRock**, il cuore di **Hyperion**, una macchina acquisitrice di territori fisici e digitali in orbita. Scritto in Rust, integra una rete P2P basata su `libp2p`, consenso Proof of Authority (PoA), e API REST/gRPC per gestire dati orbitali come mappatura territoriale e sorveglianza. Zion-Core è una Excalibur per coordinare nodi terrestri e satellitari in un sistema blockchain sicuro e scalabile.

## Ruolo in Hyperion
Zion-Core gestisce:
- **Rete P2P**: Connessioni sicure tramite `libp2p` v0.53.0.
- **API REST**: Endpoint `/blocks`, `/balances`, `/tron_balance`, `/health`, `/modules`.
- **gRPC**: Servizio `TransactionService` per comunicazioni ad alte prestazioni.
- **Metriche**: Integrazione con Prometheus.
- **Integrazione TronGrid**: Connessione al Nile Testnet per operazioni blockchain.

## Prerequisiti
- **Rust (nightly)**: `rustup install nightly`
- **Cargo**: `cargo +nightly`
- **Node.js** (opzionale, per frontend): `>= 16.x`
- **Android NDK** (opzionale, per moduli Android): `r26b`
- **Chiave TronGrid** (Nile Testnet): Ottieni una chiave gratuita su [TronGrid](https://www.trongrid.io/)
- **Docker** (opzionale): `>= 20.x`
- **`grpcurl`**: Per testare gRPC

## Installazione
1. Clona il repository principale:
   ```bash
   git clone git@github.com:BlockRockAdmin/BlockRock.git
   cd BlockRock/zion-core

# Configura la chiave TronGrid

```bash
export TRONGRIDAPIKEY=la_tua_api_key
```

# Compila

```bash
cargo +nightly build --release --bin node
```

# Esegui il nodo

```bash
cargo +nightly run --release --bin node
```

# TestREST

```bash
curl http://localhost:8000/health
curl http://localhost:8000/blocks
curl http://localhost:8000/balances
curl http://localhost:8000/tron_balance
curl http://localhost:8000/modules
```

## Output atteso:
- `/health`: OK
- `/blocks`: [] o lista di blocchi JSON
- `/balances`: [] o lista di tuple (indirizzo, saldo)
- `/tron_balance`: 100.0 (placeholder)
- `/modules`: Contenuto di modules/blockchain/modules.yaml

# gRPC

```bash
grpcurl -plaintext -d '{"id": "test"}' localhost:50051 blockrock.TransactionService/GetTransaction
```

## Output atteso:
```json
{
  "id": "test",
  "sender": "satellite_1",
  "recipient": "ground_station",
  "amount": 100.0
}
```

# Dashboard Web:
Apri [http://localhost:8000/static/index.html](http://localhost:8000/static/index.html) (o usa l’IP della macchina per accesso remoto).

# Struttura

```plaintext
zion-core/
├── Cargo.toml
├── build.rs
├── modules/
│   └── blockchain/
│       └── modules.yaml
├── proto/
│   └── zion.proto
├── src/
│   ├── api/
│   │   ├── mod.rs
│   │   ├── rest.rs
│   │   ├── grpc.rs
│   │   └── prometheus.rs
│   ├── bin/
│   │   └── node.rs
│   ├── config.rs
│   ├── network/
│   │   └── p2p.rs
│   └── main.rs
├── static/
│   └── index.html
```

## Troubleshooting

### NaN TRX nella dashboard:
* Verifica `TRONGRIDAPIKEY`: `export TRONGRIDAPIKEY=la_tua_api_key`.
* Controlla la validità dell’indirizzo TRON.

### Errore “static non trovato”:
* Crea `zion-core/static/index.html`.

### Errore di build su Rocket/libp2p:
* Usa Rust 1.76+: `rustup update nightly`.
* Verifica `Cargo.toml`:

```toml
[dependencies]
libp2p = "0.53.0"
rocket = "0.5.1"
```

### Errore “Large files detected” su GitHub:
* Escludi `target/` in `.gitignore` e rimuovi dalla storia:

```bash
git filter-repo --path zion-core/target/ --path blockrock-core/target/ --invert-paths --force
```

## Contribuire

Consulta `../CONTRIBUTING.md` per linee guida. 

In breve:
1. Fai un fork del repository.
2. Crea un branch: `git checkout -b feature/nome-feature`.
3. Testa: `cargo +nightly build --release --bin node`.
4. Commit: `git commit -m "Aggiunta feature XYZ"`.
5. Push: `git push origin feature/nome-feature`.
6. Apri una Pull Request.

## Licenza

MIT License (`../LICENSE.md`)

Contatti  
Email: team@blockrock.dev  
Sito: blockrock.dev  
GitHub Issues: BlockRock Issues  
GitHub Sponsors: Supporta il progetto  

> "La fenice di Zion-Core orchestra la sinfonia di Hyperion, brandendo Excalibur per conquistare territori orbitali!"  
