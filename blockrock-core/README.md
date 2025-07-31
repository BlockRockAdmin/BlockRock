# BlockRock Core

Libreria Rust che implementa le primitive blockchain utilizzate dall'intero progetto BlockRock. Fornisce strutture per blocchi, transazioni e gestione dei bilanci, servendo come base per il nodo e i servizi di orchestrazione.

## Compilazione

All'interno della cartella `blockrock-core` eseguire:

```bash
cargo build
```

Il modulo fa parte del workspace Rust del repository, quindi viene compilato automaticamente anche avviando `cargo build` dalla radice.

## Integrazione con il progetto

`blockrock-core` è importato da `zion-core` e da altri servizi per esporre funzionalità blockchain comuni. Le librerie generate possono essere riutilizzate da applicazioni Rust esterne o da moduli aggiuntivi della piattaforma.
