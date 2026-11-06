# Nexus · Il tuo spazio

MVP web di BlockRock per passi quotidiani volontari, interessi, obiettivi
personali e pianificazione della stabilità. Account privati e sincronizzazione
tra dispositivi, con dati separati per utente. Interfaccia adattata a desktop e
telefono.

## Cosa puoi fare

- Registrare, cercare, modificare e rimuovere gesti `+1`, annullando l'ultima modifica.
- Scegliere interessi e obiettivi, cambiare il prossimo passo, concluderli o riaprirli.
- Tenere separati gli obiettivi personali da quelli di stabilità economica.
- Mettere in pausa e riprendere il percorso senza perdere i dati.
- Accedere da più dispositivi e sincronizzare automaticamente ogni 15 secondi,
  quando la pagina è visibile, oppure con «Sincronizza ora».
- Conservare una bozza locale durante un'interruzione di rete e risolvere
  esplicitamente i conflitti. Le scritture non sovrascrivono revisioni più recenti.
- Esportare il percorso in JSON; controllare un backup e aggiungerlo o sostituirlo.
- Importare, solo su scelta esplicita, i passi del vecchio prototipo locale.
- Recuperare l'accesso con un codice personale, cambiare password, eliminare l'account.

Un `+1` non è un voto e non modifica capitale o rischio. Nexus non invia ordini
finanziari e non attribuisce diagnosi. Le regole del percorso di accumulo con
GIANO richiedono una specifica distinta.

## Avvio sul PC personale

Requisiti: Node.js 22.13 o successivo, npm e Python 3.12 o successivo con venv.
Dalla cartella `nexus`:

```bash
npm ci
python3 -m venv server/.venv
server/.venv/bin/pip install -r server/requirements.txt
npm run build
npm run start:server
```

Apri **http://127.0.0.1:8788**. Al primo avvio viene creato il codice di invito
in `server/data/invitation.txt`, leggibile solo dall'utente del servizio. Usalo
in «Crea account», scegli nome utente e password e **conserva il codice di
recupero** mostrato dopo la registrazione. Non è previsto l'invio di email.

Il servizio ascolta soltanto su loopback. I dati sono in
`server/data/nexus.sqlite3`; codice di invito, database e ambiente virtuale
sono esclusi da Git. Dopo la creazione degli account, puoi chiudere le
registrazioni avviando con `NEXUS_INVITATION='' npm run start:server`.

Per l'avvio automatico è disponibile il modello
[deploy/nexus.service.example](deploy/nexus.service.example). Dopo aver controllato
il percorso, copialo in `~/.config/systemd/user/blockrock-nexus.service`, esegui
`systemctl --user daemon-reload` e `systemctl --user enable --now blockrock-nexus`.
Il modello non viene installato automaticamente. L'avvio dopo il riavvio del PC
senza login richiede la configurazione del linger da parte dell'amministratore.

## Più dispositivi: stesso servizio, HTTPS e accesso privato

La sincronizzazione funziona accedendo allo **stesso server** con lo stesso
account: due installazioni indipendenti non si sincronizzano tra loro.

Per telefono o altri PC, usa un reverse proxy HTTPS raggiungibile soltanto nella
rete privata/VPN (per esempio Tailscale Serve), inoltrato a
`http://127.0.0.1:8788`. Avvia Nexus impostando `NEXUS_ORIGIN` all'indirizzo HTTPS
esatto del proxy, senza percorso, per esempio `https://nexus.rete-privata.example`.
Tutto deve passare dallo stesso indirizzo, incluse le richieste `/api`.
Con HTTPS la sessione usa cookie `Secure`. Non serve CORS e l'API non lo abilita.
L'HTTP remoto è rifiutato; l'eccezione HTTP vale solo per localhost/loopback.

Dominio, VPN e certificato dipendono dal server scelto e non sono attivati da
questo repository. La pubblicazione statica della sola cartella `dist` non è
sufficiente per account e sincronizzazione. Non esporre l'API direttamente a Internet.

Variabili del servizio:

| Variabile | Significato |
| --- | --- |
| `NEXUS_ORIGIN` | Origine esatta del browser; avvio normale: `http://127.0.0.1:8788` |
| `NEXUS_DATABASE` | Percorso SQLite; default `server/data/nexus.sqlite3` |
| `NEXUS_INVITATION` | Invito di almeno 16 caratteri; vuoto chiude le registrazioni. Se assente, `server.run` crea/riusa il file privato |

Un solo processo Uvicorn: SQLite serializza le scritture e la limitazione dei
tentativi di accesso è in memoria. Il proxy deve mantenere il limite di 2 MB sul
corpo delle richieste e non deve pubblicare file del database o del codice sorgente.

## Sviluppo

Avvia API e Vite in due terminali dalla cartella `nexus`:

Se hai già attivo il servizio utente sulla porta 8788, fermalo prima con
`systemctl --user stop blockrock-nexus`; riavvialo al termine dello sviluppo.

```bash
NEXUS_ORIGIN=http://127.0.0.1:5173 npm run start:server
npm run dev
```

Vite inoltra `/api` al servizio locale. Il router e l'entrypoint TypeScript sono
espliciti: i vecchi widget di wallet/MQTT e il prototipo `RecoverySteps.vue`
restano nel repository ma non fanno parte dell'applicazione attiva.
`npm run preview` avvia lo stesso servizio completo, dopo la build: gli account
richiedono anche l'API, quindi una semplice anteprima statica non basta.

## Verifiche

```bash
server/.venv/bin/pip install -r server/requirements-test.txt
npm test
npm run test:api
npx playwright install chromium
npm run test:e2e
```

I test browser compilano l'applicazione e avviano il servizio di produzione su
loopback con un database sintetico temporaneo. Verificano due sessioni browser,
bozze offline, conflitti, pausa, obiettivi, migrazione, backup JSON, recupero
account, cambio password, cancellazione e viewport mobile. È possibile usare
Chrome già installato con `NEXUS_CHROME_PATH=/usr/bin/google-chrome`.

## Dati e sicurezza

Password protette con scrypt; token di sessione e codici di recupero salvati
solo come hash. Sessioni HttpOnly/SameSite, controllo dell'origine e token CSRF,
limiti sulle richieste e sui tentativi di accesso. Cambio password e recupero
revocano gli altri accessi e ruotano il codice di recupero. Il codice nuovo viene
mostrato soltanto dopo queste operazioni.

Il percorso è conservato sul server: **non è cifrato da dispositivo a
dispositivo** ed è leggibile dall'amministratore e nei backup. I dettagli
personali non sono scritti sulla blockchain. Una bozza non sincronizzata rimane
nel localStorage dell'account; un dispositivo condiviso richiede un profilo browser
privato. Una sessione scaduta richiede nuovamente l'accesso. L'uscita è bloccata
finché ci sono modifiche da sincronizzare o conflitti da risolvere.

Il ripristino sostitutivo può rimuovere dati: esporta prima la versione attuale.
I backup JSON non includono account o password; il backup del server li include.

Il backup automatico locale usa `deploy/blockrock-nexus-backup.service` e
`deploy/blockrock-nexus-backup.timer`: crea una copia SQLite verificata al giorno
in `~/Documenti/Backup-Nexus`, con permessi privati. Le copie precedenti non
vengono cancellate automaticamente. Essendo sullo stesso PC, non proteggono da
un guasto del disco: conservarne anche una copia su un dispositivo separato.
Il timer recupera un'esecuzione mancata al successivo avvio della sessione.
L'eliminazione dell'account revoca gli accessi e rimuove i record attivi, ma
non cancella automaticamente backup, file esportati o bozze negli altri browser.

## Backup completo e prova di ripristino

Crea una copia consistente anche mentre il servizio è attivo:

```bash
server/.venv/bin/python -m server.backup server/data/nexus.sqlite3 /percorso/privato/nexus-backup.sqlite3
```

La destinazione deve essere nuova, avere una cartella esistente e stare su un
disco/copia separata dal database. Il comando usa l'API SQLite di backup,
controlla l'integrità e crea il file con permessi `0600`. Un semplice `cp` del
database attivo può tralasciare i dati nel WAL: usa il comando di backup.
La copia include account, password sotto forma di hash, sessioni e tutti i percorsi.
Per chiudere le sessioni anche nel backup prima di un ripristino, cancellane
la tabella `sessions` tramite SQLite sul file di copia.

Per verificare una copia, avvia un servizio di prova su un'altra porta con il file
copiato e una directory di build `dist` disponibile:

```bash
NEXUS_DATABASE=/percorso/privato/copia-di-prova.sqlite3 NEXUS_ORIGIN=http://127.0.0.1:8789 NEXUS_INVITATION='' server/.venv/bin/python -m uvicorn server.app:create_app --factory --host 127.0.0.1 --port 8789 --no-access-log
```

Accedi a `http://127.0.0.1:8789`, controlla passi e obiettivi ed esporta il
percorso. La suite API esegue già questa verifica in memoria su un backup reale.
Per sostituire il database principale: ferma il servizio, conserva una copia
completa dello stato attuale tramite il comando di backup, metti da parte anche
i suoi file `-wal` e `-shm` se presenti, quindi colloca il backup al percorso
configurato con proprietà e permessi corretti. Riavvia solo dopo il controllo
d'integrità. Ripristinare una copia precedente riporta anche password e
revoche delle sessioni al momento del backup: svuota `sessions` prima del riavvio.
