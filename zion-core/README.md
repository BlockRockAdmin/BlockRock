## Test
- REST: `curl http://localhost:8000/health`
- gRPC: `grpcurl -plaintext -d '{"id": "<genesis_id>"}' 192.168.8.236:50051 blockrock.TransactionService/GetTransaction`
