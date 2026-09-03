use blockrock_core::blockchain::Blockchain;
use rocket::routes;
use rocket::tokio::sync::broadcast::Sender;
use rocket::{Build, Rocket};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::api::prometheus::init_metrics;
use crate::api::rest::{
    get_account, get_balances, get_blocks, get_modules, health, metabolism_status, post_sensor,
    register_account, sensor_events, submit_transaction, tron_balance, SensorReading, TronService,
};
use crate::identity::NodeIdentity;
use crate::monitoring::SharedMetabolicStatus;
use crate::storage::ChainStore;

/// Everything the HTTP layer needs. Assembling the server here (instead of
/// inside the node binary) is what lets integration tests drive the real
/// routes without opening a socket.
pub struct ServerContext {
    pub blockchain: Arc<Mutex<Blockchain>>,
    pub identity: NodeIdentity,
    pub store: ChainStore,
    pub sensor_tx: Sender<SensorReading>,
    pub metabolic_status: SharedMetabolicStatus,
    pub tron_service: TronService,
}

pub fn build(context: ServerContext) -> Rocket<Build> {
    let rocket = rocket::build()
        .manage(context.blockchain)
        .manage(context.sensor_tx)
        .manage(context.metabolic_status)
        .manage(context.tron_service)
        .manage(context.identity)
        .manage(context.store)
        .mount(
            "/",
            routes![
                get_blocks,
                get_balances,
                submit_transaction,
                register_account,
                get_account,
                tron_balance,
                health,
                metabolism_status,
                get_modules,
                post_sensor,
                sensor_events
            ],
        );
    init_metrics(rocket)
}
