use anyhow::Result;
use async_nats::Message;
use clap::Parser;
use serde_json::from_slice;
use tokio_postgres::NoTls;
use tracing::{info, warn};
use trion::{encode_signal_256, Signal, SignalType};
use futures_util::StreamExt;

#[derive(Parser, Debug)]
#[command(name = "trion-signal-publisher")]
struct Config {
    #[arg(long, env = "TRION_NATS_URL", default_value = "nats://nats:4222")]
    nats_url: String,

    #[arg(long, env = "TRION_SIGNAL_SUBJECT", default_value = "trion.signals")]
    signal_subject: String,

    #[arg(long, env = "TRION_POSTGRES_URL", default_value = "postgres://trion:trion@postgres:5432/trion")]
    postgres_url: String,

    #[arg(long, env = "TRION_SIGNAL_REGISTRY", default_value = "0x0000000000000000000000000000000000000000")]
    signal_registry: String,

    #[arg(long, env = "TRION_SILENCE_REGISTRY", default_value = "0x0000000000000000000000000000000000000000")]
    silence_registry: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    let config = Config::parse();
    let (client, connection) = tokio_postgres::connect(&config.postgres_url, NoTls).await?;
    tokio::spawn(async move {
        if let Err(err) = connection.await {
            warn!(error = %err, "postgres connection error");
        }
    });

    init_db(&client).await?;

    let nats = async_nats::connect(config.nats_url.clone()).await?;
    let mut sub = nats.subscribe(config.signal_subject.clone().into()).await?;

    while let Some(message) = sub.next().await {
        if let Err(err) = handle_signal(&client, &config, message).await {
            warn!(error = %err, "failed to handle signal");
        }
    }

    Ok(())
}

async fn init_db(client: &tokio_postgres::Client) -> Result<()> {
    client
        .execute(
            "CREATE TABLE IF NOT EXISTS trion_signals (
                id SERIAL PRIMARY KEY,
                asset_id TEXT NOT NULL,
                signal_type TEXT NOT NULL,
                ts TIMESTAMPTZ NOT NULL,
                coherence DOUBLE PRECISION NOT NULL,
                confidence DOUBLE PRECISION NOT NULL,
                manipulation_flags INTEGER NOT NULL,
                limiting_layer INTEGER,
                coherence_gap DOUBLE PRECISION,
                trend TEXT,
                eta_recovery BIGINT,
                payload BYTEA NOT NULL
            )",
            &[],
        )
        .await?;

    client
        .execute(
            "CREATE TABLE IF NOT EXISTS trion_evm_outbox (
                id SERIAL PRIMARY KEY,
                registry TEXT NOT NULL,
                method TEXT NOT NULL,
                asset_id TEXT NOT NULL,
                payload BYTEA NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                sent_at TIMESTAMPTZ,
                tx_hash TEXT,
                error TEXT
            )",
            &[],
        )
        .await?;
    client
        .execute(
            \"ALTER TABLE trion_evm_outbox ADD COLUMN IF NOT EXISTS sent_at TIMESTAMPTZ\",
            &[],
        )
        .await?;
    client
        .execute(
            \"ALTER TABLE trion_evm_outbox ADD COLUMN IF NOT EXISTS tx_hash TEXT\",
            &[],
        )
        .await?;
    client
        .execute(
            \"ALTER TABLE trion_evm_outbox ADD COLUMN IF NOT EXISTS error TEXT\",
            &[],
        )
        .await?;
    Ok(())
}

async fn handle_signal(client: &tokio_postgres::Client, config: &Config, message: Message) -> Result<()> {
    let signal: Signal = from_slice(&message.payload)?;
    let payload = encode_signal_256(&signal);
    let asset_id = asset_key(&signal.asset_id);

    let (limiting_layer, coherence_gap, trend, eta_recovery) = match signal.silence.as_ref() {
        Some(details) => (
            Some(details.limiting_layer as i32),
            Some(details.coherence_gap),
            Some(format!("{:?}", details.trend)),
            Some(details.eta_recovery_blocks as i64),
        ),
        None => (None, None, None, None),
    };

    let signal_type = match signal.signal_type {
        SignalType::Signal => "SIGNAL",
        SignalType::Silence => "SILENCE",
    };
    let (registry, method) = match signal.signal_type {
        SignalType::Signal => (config.signal_registry.as_str(), "publishSignal"),
        SignalType::Silence => (config.silence_registry.as_str(), "publishSilence"),
    };

    client
        .execute(
            "INSERT INTO trion_signals (
                asset_id, signal_type, ts, coherence, confidence, manipulation_flags,
                limiting_layer, coherence_gap, trend, eta_recovery, payload
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
            &[
                &asset_id,
                &signal_type,
                &signal.timestamp,
                &signal.coherence_score,
                &signal.confidence,
                &(signal.manipulation_flags as i32),
                &limiting_layer,
                &coherence_gap,
                &trend,
                &eta_recovery,
                &payload.as_slice(),
            ],
        )
        .await?;

    client
        .execute(
            "INSERT INTO trion_evm_outbox (registry, method, asset_id, payload) VALUES ($1, $2, $3, $4)",
            &[&registry, &method, &asset_id, &payload.as_slice()],
        )
        .await?;

    info!(asset = asset_id, signal_type, "signal persisted");
    Ok(())
}

fn asset_key(asset_id: &[u8; 32]) -> String {
    let mut out = String::from("0x");
    for byte in asset_id {
        out.push_str(&format!("{:02x}", byte));
    }
    out
}
