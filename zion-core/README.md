# Sotto "Test":markdown

- Test gRPC:
  Per verificare il servizio TransactionService, esegui:
  
  ```bash
  grpcurl -plaintext -d '{"id": "bc3c36d97307bc99d4eac23943b2bde251f6861f615d68f5414becdedb7ac5ea"}' 192.168.8.236:50051 blockrock.TransactionService/GetTransaction
  ```

  O con il file proto:
  
  ```bash
  grpcurl -plaintext -import-path ~/Documents/BlockRock/zion-core/proto -proto zion.proto -d '{"id": "bc3c36d97307bc99d4eac23943b2bde251f6861f615d68f5414becdedb7ac5ea"}' 192.168.8.236:50051 blockrock.TransactionService/GetTransaction
  ```

  Output atteso:
  
  ```json
  {
    "id": "bc3c36d97307bc99d4eac23943b2bde251f6861f615d68f5414becdedb7ac5ea",
    "sender": "System",
    "recipient": "Genesis",
    "amount": 0.0
  }
  ```

  Per installare grpcurl:
  
  ```bash
  sudo apt install -y grpcurl
  ```

  O scarica da [https://github.com/fullstorydev/grpcurl/releases](https://github.com/fullstorydev/grpcurl/releases)

# Sotto "Troubleshooting":

- Gestione dei rami:
  Per evitare lavoro doppio, sincronizza sempre il ramo main:
  
  ```bash
  git checkout main
  git pull origin main
  ```

  Poi aggiorna i rami di feature:
  
  ```bash
  git checkout feature/<nome>
  git merge main
  ```

  Usa `git stash` per salvare modifiche temporanee.

  Committa:
  
  ```bash
  git add zion-core/README.md
  git commit -m "Update README with tested gRPC transaction ID"
  git push origin main
  ```
