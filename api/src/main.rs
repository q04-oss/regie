#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    regie_api::run().await;
}
