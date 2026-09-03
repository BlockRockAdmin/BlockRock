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
| `AUTHORITY_NAME` | `blockrock` | Nome dell'autorità: è **di rete**, non del nodo |
| `AUTHORITY_KEY_PATH` | `data/authority.key` | Seed ed25519 con cui il nodo sigilla i blocchi |

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
| `POST` | `/sensors`, `GET /sensors/stream` | Telemetria IoT |

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
