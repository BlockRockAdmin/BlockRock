# IoT

Questo modulo raccoglie gli script e le librerie dedicate all'integrazione con dispositivi IoT. L'obiettivo è permettere a sensori e attuatori di interagire con la blockchain e con il nodo di orchestrazione.

## Build

Al momento non sono previsti passaggi di compilazione specifici. Puoi aggiungere i tuoi componenti e gestirli con gli strumenti più adatti (Cargo, npm, Python, ecc.).

## Integrazione con il progetto

I dispositivi IoT comunicheranno con `zion-core` tramite API REST o protocolli message queue per registrare eventi on‑chain e ricevere comandi. Questo modulo fungerà da ponte tra l'hardware e la piattaforma BlockRock.
