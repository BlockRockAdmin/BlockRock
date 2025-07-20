use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{transport::Server, Request, Response, Status};
use tonic_reflection::server::Builder as ReflectionBuilder;
use blockrock_core::blockchain::Blockchain;
use blockrock::transaction_service_server::{TransactionService, TransactionServiceServer};
use blockrock::{TransactionRequest, TransactionResponse};

pub mod blockrock {
    tonic::include_proto!("blockrock");
}

const FILE_DESCRIPTOR_SET: &[u8] = include_bytes!("../blockrock_descriptor.bin");

#[derive(Clone)]
pub struct MyTransactionService {
    blockchain: Arc<Mutex<Blockchain>>,
}

#[tonic::async_trait]
impl TransactionService for MyTransactionService {
    async fn get_transaction(
        &self,
        request: Request<TransactionRequest>,
    ) -> Result<Response<TransactionResponse>, Status> {
        let id = request.into_inner().id;
        let blockchain = self.blockchain.lock().await;
        match blockchain.get_transaction(&id) {
            Some(tx) => Ok(Response::new(TransactionResponse {
                id,
                sender: tx.sender,
                recipient: tx.receiver,
                amount: tx.amount as f32,
            })),
            None => Err(Status::not_found("Transaction not found")),
        }
    }
}

pub async fn start_grpc(blockchain: Arc<Mutex<Blockchain>>, port: u16) -> Result<(), anyhow::Error> {
    let addr = format!("0.0.0.0:{}", port).parse()?;
    let service = MyTransactionService { blockchain };
    let reflection_service = ReflectionBuilder::configure()
        .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
        .build_v1()?;
    Server::builder()
        .add_service(TransactionServiceServer::new(service))
        .add_service(reflection_service)
        .serve(addr)
        .await?;
    Ok(())
}
