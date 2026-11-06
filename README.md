![BlockRock Logo](docs/Logo.jpeg)

## Un percorso verso una maggiore stabilità

BlockRock aiuta la persona a vedere come impegni, risorse e scelte incidono
sulla vita quotidiana, e a costruire un percorso verso una riserva per gli
imprevisti. Il prisma mette in evidenza anche i sacrifici: pagare una bolletta,
per esempio, può significare rinunciare a un acquisto utile per la casa.

```mermaid
flowchart LR
    G[Gateway personale: accordi, spese, casa] -->|dati scelti dalla persona| B[BlockRock locale: quadro e prisma]
    B --> P[Percorso personale e cuscinetto]
    S[Sostenitori] --> D[Sviluppo del progetto]
    D -. codice e aggiornamenti .-> B
```

Ogni persona usa il proprio gateway; nel caso di Alessandro la possibile fonte
è Cecchina Hub. Il collegamento automatico **non è ancora realizzato**. Nexus
offre una prima anteprima locale di un file scelto dalla persona: non sincronizza
quel file e non legge automaticamente il gateway.

Il progetto punta a dati privati locali, senza telemetria personale verso gli
sviluppatori o gli sponsor. L'uso di servizi esterni, inclusa un'IA remota,
richiede una scelta consapevole: i dati inviati a tali servizi lasciano il
dispositivo. I dati personali non vanno inseriti nella blockchain pubblica.
Gli sponsor sostengono lo sviluppo, non accedono ai dati delle persone.

Una riserva dipende da risorse effettivamente disponibili: blockchain e
investimenti non garantiscono rendimenti o perdite massime. La scaletta e ogni
eventuale operazione finanziaria automatica restano da definire. Approfondimenti:
[visione del prodotto](docs/visione-prodotto.md) e [formato del quadro](docs/gateway-personale.md).

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

## 📚 Documentazione Completa

Tutte le istruzioni di build, setup, troubleshooting e approfondimenti tecnici sono disponibili nel [Wiki ufficiale](https://github.com/BlockRockAdmin/BlockRock/wiki).  
Consulta il wiki per guide aggiornate, dettagli sui moduli, esempi e FAQ.

Avvio del nodo, consenso PoA, API REST e sincronizzazione fra nodi sono
descritti in [docs/nodo.md](docs/nodo.md).

Le transazioni accodate da un nodo non validatore possono raggiungere un
validatore attraverso i peer; i blocchi confermati tornano anche attraverso
i relay. Le ricevute, i limiti e la prova su tre nodi sono descritti nella
sezione «Transazioni ricevute da un nodo non validatore» della stessa guida.

La visione del prodotto e il percorso di recupero personale e finanziario sono
descritti in [docs/visione-prodotto.md](docs/visione-prodotto.md).

L'interfaccia personale [Nexus](nexus/README.md) comprende account privati,
sincronizzazione fra dispositivi, passi quotidiani, interessi e obiettivi,
pausa, esportazione e ripristino. Il servizio può essere avviato sul PC;
l'accesso degli altri dispositivi richiede un indirizzo HTTPS nella rete privata.

Il ciclo di monitoraggio omeostatico di Zion Core e l'endpoint di telemetria
`/metabolism` sono descritti in [docs/hypothalamus.md](docs/hypothalamus.md).

## 📚 Licenza

BlockRock è rilasciato sotto licenza MIT.

## 🤝 Contribuire

BlockRock è guidato dalla community e i contributi sono benvenuti (ma preferiamo le donazioni! 💸).

- Fai un fork del repository.
- Crea un branch descrittivo.
- Committa con messaggi chiari.
- Apri una Pull Request.
- Consulta il file `CONTRIBUTING.md` per dettagli.

## 📬 Contatti & Community

- Wiki: [https://github.com/BlockRockAdmin/BlockRock/wiki](https://github.com/BlockRockAdmin/BlockRock/wiki)
- GitHub Issues: [https://github.com/BlockRockAdmin/BlockRock/issues](https://github.com/BlockRockAdmin/BlockRock/issues)
- GitHub Sponsors: [https://github.com/sponsors/BlockRockAdmin](https://github.com/sponsors/BlockRockAdmin)

> Tutto il necessario per usare, contribuire e scoprire BlockRock lo trovi nel Wiki ufficiale!

---
