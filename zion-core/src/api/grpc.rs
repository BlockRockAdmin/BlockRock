use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{transport::Server, Request, Response, Status};
use tonic_reflection::server::Builder as ReflectionBuilder;

use blockrock_core::block::Block;
use blockrock_core::blockchain::Blockchain;
use blockrock_core::transaction::Transaction;

use blockrock::transaction_service_server::{TransactionService, TransactionServiceServer};
use blockrock::{
    AccountRequest, AccountResponse, BalancesRequest, BalancesResponse, BlockSummary,
    BlocksRequest, BlocksResponse, SubmitTransactionRequest, SubmitTransactionResponse,
    TransactionRequest, TransactionResponse,
};

use crate::api::rest::BlockAnnouncer;
use crate::identity::NodeIdentity;
use crate::ledger::{self, SubmitError};
use crate::storage::ChainStore;

pub mod blockrock {
    tonic::include_proto!("blockrock");
}

const FILE_DESCRIPTOR_SET: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/blockrock_descriptor.bin"));

/// Quello che serve al gRPC per fare le stesse cose della REST. Sono gli stessi
/// riferimenti che tiene il server HTTP: due porte, un solo nodo.
#[derive(Clone)]
pub struct GrpcContext {
    pub blockchain: Arc<Mutex<Blockchain>>,
    pub identity: Arc<NodeIdentity>,
    pub store: ChainStore,
    pub announcer: BlockAnnouncer,
}

#[derive(Clone)]
pub struct MyTransactionService {
    context: GrpcContext,
}

fn transaction_message(transaction: &Transaction) -> TransactionResponse {
    TransactionResponse {
        id: transaction.id.clone(),
        sender: transaction.sender.clone(),
        recipient: transaction.receiver.clone(),
        amount: transaction.amount,
        nonce: transaction.nonce,
        payload: transaction.payload.clone(),
    }
}

fn block_message(block: &Block) -> BlockSummary {
    BlockSummary {
        index: block.index,
        timestamp: block.timestamp,
        previous_hash: block.previous_hash.clone(),
        hash: block.hash.clone(),
        authority: block.authority.clone(),
        transactions: block.transactions.iter().map(transaction_message).collect(),
    }
}

#[tonic::async_trait]
impl TransactionService for MyTransactionService {
    async fn get_transaction(
        &self,
        request: Request<TransactionRequest>,
    ) -> Result<Response<TransactionResponse>, Status> {
        let id = request.into_inner().id;
        let blockchain = self.context.blockchain.lock().await;
        match blockchain.get_transaction(&id) {
            Some(transaction) => Ok(Response::new(transaction_message(&transaction))),
            None => Err(Status::not_found("Transaction not found")),
        }
    }

    async fn submit_transaction(
        &self,
        request: Request<SubmitTransactionRequest>,
    ) -> Result<Response<SubmitTransactionResponse>, Status> {
        let submission = request.into_inner();

        let signature =
            decode_signature(&submission.signature).map_err(Status::invalid_argument)?;
        let transaction = Transaction::signed_with_payload(
            submission.sender,
            submission.receiver,
            submission.amount,
            submission.nonce,
            submission.payload,
            signature,
        );

        // Stessa strada della REST: una sola implementazione delle regole.
        let queued = ledger::submit(
            transaction,
            &self.context.blockchain,
            &self.context.identity,
            &self.context.store,
            &self.context.announcer,
        )
        .await
        .map_err(|error| match error {
            SubmitError::Rejected(message) => Status::invalid_argument(message),
            // ResourceExhausted, non InvalidArgument: e' il codice che le retry
            // policy gRPC riconoscono come ritentabile.
            SubmitError::Busy(message) => Status::resource_exhausted(message),
            SubmitError::Internal(message) => Status::internal(message),
        })?;

        Ok(Response::new(SubmitTransactionResponse {
            id: queued.id,
            pending: queued.pending as u64,
        }))
    }

    async fn get_account(
        &self,
        request: Request<AccountRequest>,
    ) -> Result<Response<AccountResponse>, Status> {
        let name = request.into_inner().name;
        let blockchain = self.context.blockchain.lock().await;

        if blockchain.public_key(&name).is_none() && !blockchain.balances.contains_key(&name) {
            return Err(Status::not_found(format!("unknown account '{}'", name)));
        }

        Ok(Response::new(AccountResponse {
            balance: blockchain.balance_of(&name),
            next_nonce: blockchain.next_nonce(&name),
            public_key: blockchain
                .public_key(&name)
                .map(|key| hex::encode(key.to_bytes())),
            name,
        }))
    }

    async fn get_balances(
        &self,
        _request: Request<BalancesRequest>,
    ) -> Result<Response<BalancesResponse>, Status> {
        let blockchain = self.context.blockchain.lock().await;
        let balances: HashMap<String, u64> = blockchain.balances.clone();
        Ok(Response::new(BalancesResponse { balances }))
    }

    async fn get_blocks(
        &self,
        _request: Request<BlocksRequest>,
    ) -> Result<Response<BlocksResponse>, Status> {
        let blockchain = self.context.blockchain.lock().await;
        Ok(Response::new(BlocksResponse {
            blocks: blockchain.blocks.iter().map(block_message).collect(),
        }))
    }
}

/// Restituisce il motivo come stringa, non come `Status`: quel tipo e' grosso,
/// e qui serve solo un messaggio che il chiamante trasforma in
/// `InvalidArgument`.
fn decode_signature(signature: &str) -> Result<ed25519_dalek::Signature, String> {
    let bytes = hex::decode(signature)
        .map_err(|e| format!("signature is not valid hex: {}", e))?;
    let bytes: [u8; 64] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| "signature must be 64 bytes".to_string())?;
    Ok(ed25519_dalek::Signature::from_bytes(&bytes))
}

/// Serves the node API until `shutdown` resolves. The caller owns that future,
/// so the gRPC half stops on the same signal as the rest of the node instead of
/// outliving it.
pub async fn start_grpc(
    context: GrpcContext,
    port: u16,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<(), anyhow::Error> {
    let addr = format!("0.0.0.0:{}", port).parse()?;
    let service = MyTransactionService { context };
    let reflection_service = ReflectionBuilder::configure()
        .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
        .build_v1()?;
    Server::builder()
        .add_service(TransactionServiceServer::new(service))
        .add_service(reflection_service)
        .serve_with_shutdown(addr, shutdown)
        .await?;
    Ok(())
}
