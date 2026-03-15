use anyhow::Result;
use async_nats::Client;
use axum::{
    extract::{Path, State, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use clap::Parser;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_postgres::NoTls;
use tracing::{info, warn};
use trion::ManipulationAlert;
use futures_util::StreamExt;

#[derive(Parser, Debug)]
#[command(name = "trion-api")]
struct Config {
    #[arg(long, env = "TRION_API_BIND", default_value = "0.0.0.0:8080")]
    bind: String,

    #[arg(long, env = "TRION_POSTGRES_URL", default_value = "postgres://trion:trion@postgres:5432/trion")]
    postgres_url: String,

    #[arg(long, env = "TRION_NATS_URL", default_value = "nats://nats:4222")]
    nats_url: String,
}

#[derive(Clone)]
struct AppState {
    db: tokio_postgres::Client,
    nats: Client,
    alerts: Arc<Mutex<Vec<ManipulationAlert>>>,
}

#[derive(Serialize)]
struct SignalResponse {
    asset_id: String,
    signal_type: String,
    timestamp: chrono::DateTime<chrono::Utc>,
    coherence: f64,
    confidence: f64,
    manipulation_flags: i32,
    limiting_layer: Option<i32>,
    coherence_gap: Option<f64>,
    trend: Option<String>,
    eta_recovery: Option<i64>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    let config = Config::parse();
    let (db, connection) = tokio_postgres::connect(&config.postgres_url, NoTls).await?;
    tokio::spawn(async move {
        if let Err(err) = connection.await {
            warn!(error = %err, "postgres connection error");
        }
    });

    let nats = async_nats::connect(config.nats_url.clone()).await?;
    let alerts = Arc::new(Mutex::new(Vec::new()));
    spawn_alert_listener(nats.clone(), Arc::clone(&alerts));

    let state = AppState { db, nats, alerts };

    let app = Router::new()
        .route("/signal/:asset", get(get_signal))
        .route("/silence/:asset", get(get_silence))
        .route("/manipulation-alerts", get(get_alerts))
        .route("/live-signals", get(ws_signals))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&config.bind).await?;
    info!(bind = %config.bind, "api listening");
    axum::serve(listener, app).await?;

    Ok(())
}

async fn get_signal(State(state): State<AppState>, Path(asset): Path<String>) -> Result<Json<SignalResponse>, axum::http::StatusCode> {
    let row = state
        .db
        .query_opt(
            "SELECT asset_id, signal_type, ts, coherence, confidence, manipulation_flags, limiting_layer, coherence_gap, trend, eta_recovery
             FROM trion_signals WHERE asset_id = $1 ORDER BY id DESC LIMIT 1",
            &[&asset],
        )
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let row = row.ok_or(axum::http::StatusCode::NOT_FOUND)?;
    Ok(Json(row_to_response(row)))
}

async fn get_silence(State(state): State<AppState>, Path(asset): Path<String>) -> Result<Json<SignalResponse>, axum::http::StatusCode> {
    let row = state
        .db
        .query_opt(
            "SELECT asset_id, signal_type, ts, coherence, confidence, manipulation_flags, limiting_layer, coherence_gap, trend, eta_recovery
             FROM trion_signals WHERE asset_id = $1 AND signal_type = 'SILENCE' ORDER BY id DESC LIMIT 1",
            &[&asset],
        )
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let row = row.ok_or(axum::http::StatusCode::NOT_FOUND)?;
    Ok(Json(row_to_response(row)))
}

async fn get_alerts(State(state): State<AppState>) -> Json<Vec<ManipulationAlert>> {
    let guard = state.alerts.lock().await;
    Json(guard.clone())
}

async fn ws_signals(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws(socket, state))
}

async fn handle_ws(mut socket: axum::extract::ws::WebSocket, state: AppState) {
    let mut sub = match state.nats.subscribe("trion.signals".into()).await {
        Ok(sub) => sub,
        Err(_) => return,
    };

    while let Some(message) = sub.next().await {
        let payload = message.payload;
        if socket
            .send(axum::extract::ws::Message::Binary(payload.to_vec()))
            .await
            .is_err()
        {
            break;
        }
    }
}

fn spawn_alert_listener(nats: Client, alerts: Arc<Mutex<Vec<ManipulationAlert>>>) {
    tokio::spawn(async move {
        let mut sub = match nats.subscribe("behavior.alerts".into()).await {
            Ok(sub) => sub,
            Err(_) => return,
        };

        while let Some(message) = sub.next().await {
            if let Ok(alert) = serde_json::from_slice::<ManipulationAlert>(&message.payload) {
                let mut guard = alerts.lock().await;
                guard.push(alert);
                if guard.len() > 100 {
                    guard.remove(0);
                }
            }
        }
    });
}

fn row_to_response(row: tokio_postgres::Row) -> SignalResponse {
    SignalResponse {
        asset_id: row.get(0),
        signal_type: row.get(1),
        timestamp: row.get(2),
        coherence: row.get(3),
        confidence: row.get(4),
        manipulation_flags: row.get(5),
        limiting_layer: row.get(6),
        coherence_gap: row.get(7),
        trend: row.get(8),
        eta_recovery: row.get(9),
    }
}
