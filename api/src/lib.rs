pub async fn run() {
    let app = axum::Router::new()
        .route("/health", axum::routing::get(health));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap();
    tracing::info!("Regie API listening on :3000");
    axum::serve(listener, app).await.unwrap();
}

async fn health() -> &'static str {
    "ok"
}
