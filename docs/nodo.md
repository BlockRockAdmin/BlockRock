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
