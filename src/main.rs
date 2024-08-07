use axum::{routing::post, Json, Router};
use serde::Deserialize;
use tokio::time::{sleep, Duration};
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::{info, Level};

#[derive(Deserialize)]
struct MonitorRequest {
    url: String,
    webhook: String,
}

async fn monitor_service(Json(request): Json<MonitorRequest>) -> &'static str {
    tokio::spawn(async move {
        loop {
            if !ping_service(&request.url).await {
                send_webhook_notification(&request.webhook, &request.url).await;
            }
            sleep(Duration::from_secs(300)).await; // Check every 5 minutes
        }
    });
    info!("Monitor started");
    "Monitor started"
}

async fn ping_service(url: &str) -> bool {
    let client = reqwest::Client::new();
    match client.get(url).send().await {
        Ok(response) => response.status().is_success(),
        Err(_) => false,
    }
}

async fn send_webhook_notification(webhook_url: &str, service_url: &str) {
    let client = reqwest::Client::new();
    let payload = serde_json::json!({
        "text": format!("🚨 {} is down! 🚨", service_url)
    });

    let _ = client.post(webhook_url).json(&payload).send().await;
}

#[tokio::main]
async fn main() {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let app = Router::new()
        .route("/monitor", post(monitor_service))
        .layer(ServiceBuilder::new().layer(TraceLayer::new_for_http()));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
