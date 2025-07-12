![BlockRock Logo](docs/Logo.jpeg)

## 📁 Struttura del repository

```
BlockRock/
├── .github/
│   └── workflows/        # Workflow CI/CD automatizzati (build, test, deploy)
│       └── deploy.yml    # Esempio di workflow per il deploy automatico
├── blockrock-core/       # Modulo blockchain: nodo P2P, consenso PoA, storage decentralizzato
│   └── src/              # Codice sorgente Rust del nodo blockchain
│       └── ...           # Altri file e moduli interni
├── zion-core/            # Modulo orchestratore: API REST/gRPC, metriche, integrazione Tron
│   └── src/              # Codice sorgente Rust dell’orchestratore
│       └── ...           # Altri file e moduli interni
├── static/               # File statici per dashboard web e UI (HTML, JS, CSS)
│   └── index.html        # Home della dashboard web
├── docs/                 # Documentazione, immagini, diagrammi e materiale di supporto
│   ├── img/              # Immagini e loghi del progetto
│   │   └── Logo.jpg      # Logo ufficiale BlockRock
│   └── ...               # Altri file di documentazione
├── iot/                  # Modulo per dispositivi e integrazione IoT
├── lifeforge/            # Modulo app mobile (Android) e gamification
├── ml/                   # Moduli e risorse per Machine Learning e AI
├── nexus/                # Dashboard avanzata, UI e integrazione dati real-time
├── .dockerignore         # File per escludere elementi dal build Docker
├── .gitignore            # File per escludere file/cartelle dal versionamento Git
├── README.md             # Documentazione principale del progetto
├── LICENSE.md            # Licenza MIT del progetto
├── CONTRIBUTING.md       # Linee guida per contribuire al progetto
├── run_node.sh           # Script di avvio rapido per zion-core
├── summary.sh            # Script per generare un riepilogo dello stato del progetto
├── Cargo.lock            # Lockfile delle dipendenze Rust
├── Cargo.toml            # Configurazione e dipendenze Rust del progetto
```

---

## 📝 Build & Setup

### Prerequisiti

- [Rust 1.76+](https://rustup.rs/)
- [Node.js](https://nodejs.org/) (solo per sviluppo frontend avanzato)
- [Android NDK r26b](https://developer.android.com/ndk/downloads) (per moduli Android)
- **Chiave TronGrid (Nile Testnet):**  
  Ottieni la tua API key gratuita su [TronGrid](https://www.trongrid.io/)

### Setup rapido

`git clone https://github.com/BlockRockAdmin/BlockRock.git`  
`cd BlockRock`

Build blockrock-core (blockchain)  
`cd blockrock-core`  
`cargo build`

Build e lancia zion-core (orchestratore)  
`cd ../zion-core`  
`cargo build`  
`export TRONGRIDAPIKEY=la_tua_api_key` `# Imposta la chiave TronGrid`  
`cargo run`

text

**Dashboard web:**  
Apri [http://localhost:8000/static/index.html](http://localhost:8000/static/index.html)  
(o sostituisci `localhost` con l’IP della tua macchina se accedi da remoto).

---

## 🛠️ Troubleshooting

- **NaN TRX nella dashboard:**  
  Verifica che la variabile d’ambiente `TRONGRIDAPIKEY` sia impostata e che l’indirizzo TRON sia valido.
- **Errore “static non trovato” all’avvio:**  
  Crea la cartella `static/` dentro `zion-core` e inserisci `index.html`.
- **Errore di build su Rocket/libp2p:**  
  Assicurati di usare Rust 1.76+ e le versioni indicate in `Cargo.toml`.

---

## 📚 Licenza

BlockRock è rilasciato sotto licenza [MIT](https://github.com/BlockRockAdmin/BlockRock/blob/main/LICENSE.md).

---

## 🤝 Contribuire

BlockRock è guidato dalla community e **i contributi sono benvenuti** (ma preferiamo le donazioni! [💸](https://github.com/sponsors/BlockRockAdmin)).

1. Fai un fork del repository.
2. Crea un branch descrittivo (es. `feature-miglioramenti-gps`).
3. Committa con messaggi chiari.
4. Apri una Pull Request.
5. Consulta il file [CONTRIBUTING.md](https://github.com/BlockRockAdmin/BlockRock/blob/main/CONTRIBUTING.md) per dettagli.

---

## 📬 Contatti & Community

- [Wiki](https://github.com/BlockRockAdmin/BlockRock/wiki)
- [GitHub Issues](https://github.com/BlockRockAdmin/BlockRock/issues)
- [GitHub Sponsors](https://github.com/sponsors/BlockRockAdmin)

---

> BlockRock: Forgia il tuo destino nella blockchain sostenibile!
