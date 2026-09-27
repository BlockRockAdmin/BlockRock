use prometheus::Registry;
use rocket::{Build, Rocket};

pub fn init_metrics(rocket: Rocket<Build>) -> Rocket<Build> {
    let registry = Registry::new();
    rocket.manage(registry)
}
