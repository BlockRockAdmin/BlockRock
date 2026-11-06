# Roadmap di BlockRock

Stato verificato il 5 novembre 2026. La visione di riferimento è in
[visione-prodotto.md](visione-prodotto.md).

| Passaggio | Stato | Risultato e criterio di chiusura |
| --- | --- | --- |
| Nodo e rete di prova | Prototipo funzionante | Consenso PoA, sincronizzazione dei blocchi e inoltro delle transazioni anche tramite relay. Restano separati persistenza della coda e preparazione all'esercizio stabile. |
| Nexus: percorso personale | MVP completato | Passi modificabili e annullabili, interessi, obiettivi personali e di stabilità separati, pausa e ripresa. |
| Nexus: account e dati | MVP completato | Accesso, recupero, cambio password, cancellazione, isolamento fra account, sincronizzazione con controllo delle revisioni, bozze locali ed esportazione/ripristino. |
| Nexus sul PC | Attivato | Servizio utente su loopback, avvio al login, pagina e API controllate nel browser. |
| Accesso privato da altri dispositivi | Preparato; attivazione rimandata | Sincronizzazione verificata con due sessioni browser. L'utente ha scelto di usare per ora il solo PC. Per telefono/altri PC serve l'indirizzo HTTPS del servizio nella rete privata. |
| Backup automatici fuori dal disco del PC | Da configurare | Destinazione separata, pianificazione, conservazione e prova periodica. Il comando di backup consistente e il test di ripristino con account sono già disponibili. |
| Percorso economico con GIANO | Da specificare | Approvare livelli, transizioni e limiti prima delle integrazioni operative. I passi personali non determinano capitale, rischio o ordini. |
| App Android e altre integrazioni | Successivi | Partire dai flussi verificati in Nexus; i moduli IoT, ML e LifeForge richiedono ancora una definizione concreta. |

## Verifica del primo MVP Nexus

- 15 test sui dati: validazione, migrazione, esportazione, unione e limiti.
- 14 test del servizio: account, permessi, sessioni, CSRF, ripristino, scritture
  concorrenti, recupero e input Unicode.
- 8 scenari browser: percorso personale, due dispositivi, conflitti, bozze offline,
  ripristino, account, mobile, storage pieno e due schede.
- Compilazione TypeScript/Vite e lint dei file modificati riusciti con Node 22.
- Installazione ripetuta con `npm ci`; audit npm senza vulnerabilità segnalate.

La verifica automatica è inserita anche nel job CI Nexus. Il suo esito su GitHub
si potrà controllare dopo il caricamento delle modifiche. Codice Rust e servizi
GIANO non sono stati modificati da questa consegna.

I dettagli di avvio, dati privati e backup sono nella [guida Nexus](../nexus/README.md).
