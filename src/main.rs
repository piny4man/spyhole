use axum::{
    extract::State,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::env;
use tokio::time::{sleep, Duration};
use tower::ServiceBuilder;
use tower_http::{services::ServeDir, trace::TraceLayer};
use tracing::{info, Level};

#[derive(Deserialize)]
struct MonitorRequest {
    url: String,
    webhook: String,
}

#[derive(Serialize)]
struct MonitorResponse {
    id: i32,
    url: String,
    webhook: String,
    last_checked: Option<NaiveDate>,
    status: Option<bool>,
}

async fn start_monitoring(pool: PgPool, id: i32, url: String, webhook: String) {
    let monitored_url = url.clone();
    tokio::spawn(async move {
        loop {
            let status = ping_service(&url).await;
            let current_date = Utc::now().date_naive();

            sqlx::query!(
                "UPDATE monitored_urls SET last_checked = $1, status = $2 WHERE id = $3",
                current_date,
                status,
                id
            )
            .execute(&pool)
            .await
            .expect("Failed to update URL");

            if !status {
                send_webhook_notification(&webhook, &url).await;
            }
            sleep(Duration::from_secs(900)).await; // Check every 15 minutes
        }
    });
    info!("Monitor for {} started", monitored_url);
}

async fn monitor_service(
    State(pool): State<PgPool>,
    Json(request): Json<MonitorRequest>,
) -> impl IntoResponse {
    let result = sqlx::query!(
        "INSERT INTO monitored_urls (url, webhook) VALUES ($1, $2) RETURNING id, url, webhook, last_checked, status",
        request.url,
        request.webhook
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to insert URL");

    let id = result.id;
    let url = result.url.clone();
    let webhook = result.webhook.clone();

    start_monitoring(pool.clone(), result.id, url, webhook).await;

    Json(MonitorResponse {
        id,
        url: result.url,
        webhook: result.webhook,
        last_checked: result.last_checked,
        status: result.status,
    })
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

async fn get_monitored_urls(State(pool): State<PgPool>) -> impl IntoResponse {
    let monitored_urls = sqlx::query_as!(
        MonitorResponse,
        "SELECT id, url, webhook, last_checked, status FROM monitored_urls"
    )
    .fetch_all(&pool)
    .await
    .expect("Failed to fetch monitored URLs");

    let mut response_html = String::new();
    for url in monitored_urls {
        response_html.push_str(&format!(
            "<li>ID: {} - URL: {} - Status: {}</li>",
            url.id,
            url.url,
            url.status.unwrap_or(false)
        ));
    }

    response_html.into_response()
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to create pool");

    let active_monitored_urls = sqlx::query!("SELECT id, url, webhook FROM monitored_urls")
        .fetch_all(&pool)
        .await
        .expect("Failed to fetch monitored URLs");

    for url in active_monitored_urls {
        start_monitoring(pool.clone(), url.id, url.url, url.webhook).await;
    }

    let serve_static = ServeDir::new("static");

    let app = Router::new()
        .route("/monitor", post(monitor_service))
        .route("/monitored_urls", get(get_monitored_urls))
        .nest_service("/", serve_static.clone())
        .fallback_service(serve_static)
        .layer(ServiceBuilder::new().layer(TraceLayer::new_for_http()))
        .with_state(pool);

    let port = env::var("APP_PORT").unwrap_or_else(|_| "5000".to_string());
    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
