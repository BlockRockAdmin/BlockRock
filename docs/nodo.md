# Il nodo BlockRock

Il binario `node` di `zion-core` tiene insieme quattro cose: la catena
persistita su disco, l'API REST, il server gRPC e la rete P2P.

## Avvio

```bash
cp .env.example .env      # TRONGRIDAPIKEY e TRON_ADDRESS sono obbligatorie
cargo run --bin node
```

Al primo avvio il nodo genera la chiave dell'autorità in `data/authority.key`
(permessi `0600`) e crea la catena in `data/chain.json`. Agli avvii successivi
ricarica la catena, la **valida per intero** e riparte dalla stessa punta: non
si riparte più da zero a ogni riavvio.

| Variabile | Default | A cosa serve |
| --- | --- | --- |
| `CHAIN_PATH` | `data/chain.json` | Dove vive la catena |
| `AUTHORITY_NAME` | `blockrock` | Nome con cui il nodo sigilla. Entra nel genesis |
| `AUTHORITY_KEY_PATH` | `data/authority.key` | Seed ed25519 con cui il nodo sigilla i blocchi |
| `INITIAL_AUTHORITIES` | *(vuoto)* | Altre autorità alla genesi, `nome:pubkey_hex` separate da virgola |
| `METABOLIC_WEBHOOK` | *(vuoto)* | Dove spedire le decisioni del monitoraggio |

## Autorità multiple

Di default la rete ha una sola autorità e tutti i nodi condividono lo stesso
`AUTHORITY_KEY_PATH`. Per una rete con più validatori, ogni nodo tiene la
propria chiave e viene autorizzato dagli altri.

Il set di autorità si fissa **alla genesi**, tramite `INITIAL_AUTHORITIES`:

```bash
# sul nodo che crea la catena
INITIAL_AUTHORITIES=bob:d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a
```

La chiave pubblica di un nodo si legge dal nodo stesso:

```bash
curl -s localhost:8000/accounts/$AUTHORITY_NAME | jq -r .public_key
```

Da lì in poi il set viaggia da solo: i nodi se lo scambiano insieme alla catena
durante il sync, e un'autorità viene accettata solo se la sua chiave pubblica è
già nota — non si impara un nome e una chiave nello stesso passaggio.

> **Attenzione.** `INITIAL_AUTHORITIES` viene letto **solo quando la catena
> viene creata**. Se `data/chain.json` esiste già, il valore è ignorato in
> silenzio: aggiungere un'autorità a un nodo già avviato non produce alcun
> effetto e nessun avviso. Per cambiare il set su una catena esistente bisogna
> ripartire dalla genesi.

Nota che `AUTHORITY_NAME` entra nel blocco di genesis: due nodi che partono
entrambi da zero con nomi diversi producono genesis diversi e non si
sincronizzeranno. I nodi secondari devono adottare via sync la catena di chi
l'ha creata.

### Letture ancorate

`POST /sensors` è la telemetria viva: la lettura arriva, viene ritrasmessa su
`/sensors/stream` e sparisce. Nulla la trattiene.

`POST /sensors/commit` è l'altra metà. La lettura diventa una transazione a
importo **zero** dal sensore verso sé stesso: non muove valore, ma il dato
resta in catena firmato, e chi lo rilegge può verificarlo con la chiave
pubblica del sensore.

Il sensore va prima registrato come un conto qualunque, con `POST /accounts`.
Poi firma, esattamente come per un trasferimento, ma con il valore nel payload:

```json
POST /sensors/commit
{
  "sensor_id": "termometro",
  "value": "22.5",
  "nonce": 0,
  "signature": "<hex>"
}
```

Il valore viaggia come **stringa**, non come numero: la firma copre quei byte,
e riformattare `22.5` in `22.50` — o un `1.0` in `1` — darebbe byte diversi e
una firma invalida. Il nodo memorizza esattamente ciò che è stato firmato.

Una lettura alterata dopo la firma non passa: l'id della transazione è un hash
del contenuto, quindi la manomissione si vede prima ancora di controllare la
firma.

## Monitoraggio metabolico

L'ipotalamo campiona CPU, memoria, latenza ed errori ogni 15 secondi e decide
fra tre risposte: `hypertrophy` (serve più capacità), `atrophy` (ne avanza),
`maintain`. Entrambe le decisioni di cambiamento arrivano solo dopo che lo
stato si è confermato per più cicli: un picco isolato non muove niente, e un
istante di quiete non toglie capacità che serve un momento dopo.

Il nodo **non aziona nulla da sé**, per scelta: non ha credenziali cloud e non
tocca il socket Docker. Le decisioni sono leggibili su `GET /metabolism`, e se
imposti `METABOLIC_WEBHOOK` vengono anche spedite in POST JSON a quell'indirizzo.

```bash
METABOLIC_WEBHOOK=http://localhost:9000/metabolismo
```

Il corpo è lo stesso oggetto di `/metabolism`: azione, punteggio di stress,
cicli e vitali. Due dettagli che contano in esercizio:

- La notifica parte **solo quando la decisione cambia**. A un tick ogni 15
  secondi, rimandare `maintain` all'infinito sarebbe rumore; il segnale utile è
  il momento della transizione — rientro alla normalità compreso.
- Un webhook irraggiungibile **non ferma il monitoraggio**: l'errore viene
  registrato e il loop prosegue. La chiamata ha un timeout di 5 secondi, così
  un endpoint lento non trattiene il ciclo.

## API gRPC

Sulla porta **50051**, con reflection attiva (quindi `grpcurl` la esplora da
sé). Rispecchia la REST, e non per simmetria: entrambe le porte passano per la
stessa funzione di invio, così una regola come il divieto di conio vale su
tutte e due senza doverla scrivere due volte.

| Metodo | Cosa fa |
| --- | --- |
| `SubmitTransaction` | Invia un trasferimento o una lettura firmata |
| `GetTransaction` | Una transazione per id |
| `GetAccount` | Saldo, prossimo nonce e chiave pubblica |
| `GetBalances` | Tutti i saldi |
| `GetBlocks` | La catena completa |

```bash
grpcurl -plaintext localhost:50051 list blockrock.TransactionService
grpcurl -plaintext -d '{"name":"Alice"}' localhost:50051 \
  blockrock.TransactionService/GetAccount
```

Il campo `payload` è `optional` nel proto proprio per distinguere una
transazione senza dato da una con dato vuoto. Gli errori seguono la stessa
divisione della REST: `InvalidArgument` quando la richiesta è malfatta — firma,
nonce, fondi — e `Internal` quando è il nodo a non farcela.

## Mempool e formazione dei blocchi

Una transazione accettata **non** viene sigillata subito: entra nel mempool e
aspetta. I blocchi si formano in due modi:

- **Il tick periodico**, ogni 5 secondi, svuota il mempool in un blocco.
- **La soglia**: se il lotto in attesa supera i 64 KiB, l'invio sigilla
  all'istante senza aspettare il tick, così una raffica non paga la latenza.

Prima la sigillatura avveniva a *ogni* invio, quindi ogni transazione otteneva
un blocco tutto suo e il tick trovava sempre il mempool vuoto: il batching non
succedeva mai.

La soglia è in **byte**, non in numero di transazioni, perché da quando esiste
il payload le transazioni non hanno più dimensione uniforme: contarle
misurerebbe la cosa sbagliata.

### I due tetti

Accumulare invece di sigillare subito rende necessari due limiti che prima non
servivano, perché il mempool non conteneva mai più di un elemento:

| Limite | Valore | Perché |
| --- | --- | --- |
| Payload per transazione | 4 KiB | Nessun budget complessivo regge un singolo elemento più grande del budget |
| Mempool | 1 MiB | Accumulare senza tetto è memoria illimitata offerta a chi sa firmare |

Il tetto sul payload è una **regola di consenso**, non un controllo dell'API:
vale anche per i blocchi che arrivano dai peer, altrimenti basterebbe entrare
dalla porta P2P per aggirarlo.

### Sequenze dallo stesso mittente

Un mittente può accodare più transazioni di fila senza aspettare un blocco fra
l'una e l'altra: i nonce proseguono contando anche ciò che è già in coda, e i
fondi disponibili sono il saldo in catena meno quanto è già accodato. Una
sequenza con un buco resta rifiutata.

Per vederlo funzionare:

```bash
cargo run --example sensor_client
```

Firma cinque letture di fila e verifica che producano un blocco solo.

## Consenso

Proof of Authority vero: ogni blocco oltre al genesis porta una firma
dell'autorità che lo ha prodotto, e la validazione rifiuta blocchi non
sigillati, sigillati male o firmati da un nome non autorizzato. Il genesis
resta non firmato perché è la radice della fiducia.

Le transazioni portano un **nonce per mittente**: rigiocare una transazione già
firmata fallisce perché quel nonce è stato consumato. Gli importi sono interi
nell'unità minima, mai in virgola mobile. Saldi e nonce si muovono transazione
per transazione durante la validazione, quindi due trasferimenti nello stesso
blocco non possono spendere gli stessi fondi.

Il conto `System` conia valore: non ha firma né nonce e non viene addebitato.
Per questo l'API rifiuta qualsiasi transazione che lo indichi come mittente.

## API REST

| Metodo | Percorso | Cosa fa |
| --- | --- | --- |
| `GET` | `/blocks` | La catena completa |
| `GET` | `/balances` | Tutti i saldi |
| `GET` | `/accounts/<nome>` | Saldo e **prossimo nonce** da usare |
| `POST` | `/accounts` | Registra la chiave pubblica di un conto |
| `POST` | `/transactions` | Invia un trasferimento già firmato |
| `GET` | `/health`, `/metabolism`, `/modules` | Stato del nodo |
| `POST` | `/sensors`, `GET /sensors/stream` | Telemetria viva, effimera |
| `POST` | `/sensors/commit` | Ancora una lettura **firmata** in catena |

Il flusso completo:

```bash
# 1. registra la chiave pubblica (32 byte in hex) del conto
curl -X POST localhost:8000/accounts -H 'Content-Type: application/json' \
     -d '{"name":"Alice","public_key":"<hex 32 byte>"}'

# 2. leggi il nonce da usare
curl localhost:8000/accounts/Alice

# 3. invia la transazione firmata
curl -X POST localhost:8000/transactions -H 'Content-Type: application/json' \
     -d '{"sender":"Alice","receiver":"Bob","amount":30,"nonce":0,"signature":"<hex 64 byte>"}'
```

La firma copre il payload canonico della transazione, non il JSON. Il payload è
la concatenazione di:

```
b"blockrock.transaction.v1"
len(sender) as u64 LE || sender
len(receiver) as u64 LE || receiver
amount as u64 LE
nonce as u64 LE
```

e l'id della transazione è `sha256` dello stesso payload, quindi è il nodo a
calcolarlo: un client non può sceglierselo. In Rust:

```rust
let payload = Transaction::unsigned(sender, receiver, amount, nonce).signing_payload();
let signature = hex::encode(signing_key.sign(&payload).to_bytes());
```

Un rifiuto risponde con lo stato giusto e un JSON `{"error": "..."}` che dice
cosa non è andato (nonce sbagliato, fondi insufficienti, firma non valida).

## Sincronizzazione fra nodi

I nodi si scoprono via mDNS e parlano il protocollo `/blockrock/sync/1.0.0`
(libp2p request-response, payload JSON):

- alla prima comparsa di un peer gli si chiede `GetChain`; la sua catena viene
  adottata solo se è **più lunga**, condivide il nostro **genesis** ed è valida
  da zero (link, hash, sigilli, firme, nonce, saldi);
- ogni blocco sigillato dall'API viene annunciato con `NewBlock` a tutti i peer
  connessi. Chi lo riceve lo appende se estende la sua punta, lo ignora se lo ha
  già, e chiede la catena intera se non combacia;
- la catena viene riscritta su disco dopo ogni blocco accettato, anche quando
  arriva dalla rete.

Il genesis è deterministico (timestamp fisso, nessuna transazione con chiave
casuale), quindi due nodi con la stessa allocazione iniziale e lo stesso
`AUTHORITY_NAME` concordano sul suo hash.

Per vedere il tutto senza configurare niente:

```bash
cargo run --example sync_demo
```

## Limiti noti

- **Rete a singola autorità**: i nodi che devono accettarsi i blocchi a vicenda
  devono condividere lo stesso `data/authority.key`. Un elenco di autorità
  multiple, ognuna con la sua chiave, è il passo successivo.
- **Le chiavi dei conti non sono on-chain**: viaggiano insieme ai blocchi nei
  messaggi di sync (una registrazione già nota non viene mai sovrascritta).
  Registrarle con una transazione dedicata è la soluzione pulita.
- **Niente mempool**: ogni transazione accettata diventa subito un blocco.
- **Nessuna risoluzione dei fork oltre "la più lunga vince"**: due autorità che
  sigillano in parallelo non vengono riconciliate.
