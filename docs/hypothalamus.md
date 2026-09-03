# Ipotalamo di BlockRock

`zion-core` include un ciclo di monitoraggio omeostatico che trasforma la telemetria in una decisione, senza azionare autonomamente infrastrutture o spendere denaro.

Il nodo legge CPU e memoria da Linux `/proc`; gli adattatori futuri possono fornire latenza ed errori di rete implementando `VitalsSource`. CPU e memoria contribuiscono allo stress al 40% ciascuna; superare la soglia di latenza aggiunge una penalità e gli errori aumentano ulteriormente il punteggio.

Una sola anomalia non provoca cambiamenti. Per impostazione predefinita devono essere osservati cinque cicli consecutivi di stress per emettere `hypertrophy`, o cinque cicli di sottoutilizzo per emettere `atrophy`. In tutti gli altri casi il risultato è `maintain`.

Con il binario `node` attivo, `GET /metabolism` restituisce l'ultimo stato:

```json
{
  "action": "maintain",
  "stress_score": 42.3,
  "stress_cycles": 0,
  "relaxation_cycles": 0,
  "vitals": {
    "cpu_usage": 24.5,
    "memory_usage": 31.2,
    "latency_ms": 0.0,
    "error_rate": 0.0
  }
}
```

`hypertrophy` e `atrophy` sono richieste di capacità, scritte nel log e visibili nell'API. Un attuatore separato dovrà applicare limiti di budget, una lista esplicita di risorse consentite e un'approvazione prima di qualunque scale-up o scale-down.
