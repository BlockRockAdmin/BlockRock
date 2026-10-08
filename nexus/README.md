# Nexus

Interfaccia web di BlockRock. Il prototipo attuale permette di registrare
volontariamente piccoli gesti quotidiani come passi `+1`.

## Avvio locale

Requisiti: Node.js 22.13 o successivo e npm.

```bash
npm ci
npm run dev
```

Vite stampa l'indirizzo locale da aprire nel browser.

## Prototipo dei passi quotidiani

- La persona sceglie un gesto suggerito o ne scrive uno proprio.
- Ogni gesto viene registrato come `+1`; non è un voto e non attiva operazioni
  finanziarie.
- I dati restano nel `localStorage` del browser corrente. Non vengono inviati a
  Cecchina Hub, a Giano o alla blockchain e non si sincronizzano tra dispositivi.
- La persona può rimuovere singoli gesti dal registro.

Questo prototipo non assegna diagnosi e non contiene ancora una scaletta di
recupero o un motore di accumulo. Questi passaggi richiedono regole di prodotto
definite separatamente.
