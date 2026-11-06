# Gateway personale e quadro locale

Il gateway è una fonte scelta dalla persona: può contenere accordi, spese e
necessità, con provenienza e data. Cecchina Hub è un possibile gateway; il
collegamento automatico e il controllo degli accessi per categoria sono da
realizzare. Nessun gateway è collegato automaticamente.

Nexus offre in «Il mio quadro» un'anteprima temporanea di un file JSON scelto
manualmente. Il browser legge il file localmente; non lo invia all'API, non lo
salva nell'account e lo elimina dalla vista quando si cambia pagina o si
ricarica. Non è un backup o una sincronizzazione. Il nome della fonte e la data
sono dichiarati nel file, non verificati autonomamente. L'anteprima non calcola
saldi, non stima perdite e non dispone transazioni.

Formato iniziale (i dati sotto sono inventati):

```json
{
  "format": "blockrock-gateway-v1",
  "source": "Esempio personale",
  "updatedAt": "2026-11-06T10:00:00Z",
  "items": [
    {"title": "Anticipo per un acquisto", "kind": "impegno", "status": "previsto", "note": "In attesa di rimborso", "amountCents": 4250},
    {"title": "Acquisto per la casa", "kind": "necessita", "status": "in_corso", "note": "Rimandato", "amountCents": null}
  ]
}
```

Sono ammessi tipi `impegno`, `spesa`, `necessita`; stati `previsto`, `in_corso`,
`concluso`; importi in centesimi di euro oppure `null` se sconosciuti. Il file è
limitato a 1 MB e 500 voci; una voce non valida blocca l'intera anteprima.
Non inserire copie personali nel repository o nella blockchain.
