use anyhow::{Context, Result};
use clap::Parser;
use ethers::prelude::*;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tokio_postgres::NoTls;
use tracing::{info, warn};

abigen!(
    TrionSignalRegistry,
    r#"[
        function publishSignal(bytes32 assetId,uint64 timestamp,uint64 coherence,uint64 confidence,uint32 manipulationFlags,bytes32[8] payload)
    ]"#
);

abigen!(
    TrionSilenceRegistry,
    r#"[
        function publishSilence(bytes32 assetId,uint64 timestamp,uint64 coherence,uint64 confidence,uint8 limitingLayer,uint64 coherenceGap,uint8 trend,uint64 etaRecovery,bytes32[8] payload)
    ]"#
);

#[derive(Parser, Debug)]
#[command(name = "trion-relayer")]
struct Config {
    #[arg(long, env = "TRION_POSTGRES_URL", default_value = "postgres://trion:trion@postgres:5432/trion")]
    postgres_url: String,

    #[arg(long, env = "TRION_EVM_RPC", default_value = "http://localhost:8545")]
    evm_rpc: String,

    #[arg(long, env = "TRION_EVM_PRIVATE_KEY")]
    private_key: String,

    #[arg(long, env = "TRION_EVM_CHAIN_ID", default_value_t = 2000)]
    chain_id: u64,

    #[arg(long, env = "TRION_RELAYER_POLL_MS", default_value_t = 3000)]
    poll_ms: u64,

    #[arg(long, env = "TRION_RELAYER_BATCH", default_value_t = 10)]
    batch_size: i64,
}

#[derive(Debug)]
struct OutboxRow {
    id: i64,
    registry: String,
    method: String,
    payload: Vec<u8>,
}

#[derive(Debug)]
struct PayloadData {
    signal_type: u8,
    timestamp: u64,
    asset_id: [u8; 32],
    coherence: u64,
    confidence: u64,
    manipulation_flags: u32,
    limiting_layer: u8,
    coherence_gap: u64,
    trend: u8,
    eta_recovery: u64,
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

    let provider = Provider::<Http>::try_from(config.evm_rpc.as_str())?;
    let wallet = LocalWallet::from_str(&config.private_key)?.with_chain_id(config.chain_id);
    let client = Arc::new(SignerMiddleware::new(provider, wallet));

    loop {
        let rows = load_outbox(&db, config.batch_size).await?;
        if rows.is_empty() {
            sleep(Duration::from_millis(config.poll_ms)).await;
            continue;
        }

        for row in rows {
            if let Err(err) = process_row(&db, &client, row).await {
                warn!(error = %err, "failed to relay outbox row");
            }
        }
    }
}

async fn load_outbox(db: &tokio_postgres::Client, batch_size: i64) -> Result<Vec<OutboxRow>> {
    let rows = db
        .query(
            "SELECT id, registry, method, payload FROM trion_evm_outbox WHERE sent_at IS NULL ORDER BY id ASC LIMIT $1",
            &[&batch_size],
        )
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| OutboxRow {
            id: row.get(0),
            registry: row.get(1),
            method: row.get(2),
            payload: row.get(3),
        })
        .collect())
}

async fn process_row<M: Middleware + 'static>(
    db: &tokio_postgres::Client,
    client: &Arc<M>,
    row: OutboxRow,
) -> Result<()> {
    let registry = Address::from_str(&row.registry)
        .with_context(|| format!("invalid registry address: {}", row.registry))?;
    if registry == Address::zero() {
        mark_error(db, row.id, "registry is zero address").await?;
        return Ok(());
    }

    let payload = parse_payload(&row.payload)?;
    let chunks = payload_chunks(&row.payload)?;

    let tx_hash = if payload.signal_type == 1 {
        let contract = TrionSignalRegistry::new(registry, client.clone());
        let call = contract.publish_signal(
            payload.asset_id.into(),
            payload.timestamp,
            payload.coherence,
            payload.confidence,
            payload.manipulation_flags,
            chunks,
        );
        let pending = call.send().await?;
        let receipt = pending.await?.context("missing receipt")?;
        receipt.transaction_hash
    } else if payload.signal_type == 2 {
        let contract = TrionSilenceRegistry::new(registry, client.clone());
        let call = contract.publish_silence(
            payload.asset_id.into(),
            payload.timestamp,
            payload.coherence,
            payload.confidence,
            payload.limiting_layer,
            payload.coherence_gap,
            payload.trend,
            payload.eta_recovery,
            chunks,
        );
        let pending = call.send().await?;
        let receipt = pending.await?.context("missing receipt")?;
        receipt.transaction_hash
    } else {
        mark_error(db, row.id, "unknown signal type").await?;
        return Ok(());
    };

    db.execute(
        "UPDATE trion_evm_outbox SET sent_at = NOW(), tx_hash = $1, error = NULL WHERE id = $2",
        &[&format!("0x{:x}", tx_hash), &row.id],
    )
    .await?;

    info!(outbox_id = row.id, method = row.method, "relayed signal");
    Ok(())
}

async fn mark_error(db: &tokio_postgres::Client, id: i64, error: &str) -> Result<()> {
    db.execute(
        "UPDATE trion_evm_outbox SET error = $1 WHERE id = $2",
        &[&error, &id],
    )
    .await?;
    Ok(())
}

fn parse_payload(payload: &[u8]) -> Result<PayloadData> {
    if payload.len() < trion::SIGNAL_ENCODING_BYTES {
        anyhow::bail!("payload must be 256 bytes");
    }
    let mut asset_id = [0u8; 32];
    asset_id.copy_from_slice(&payload[9..41]);

    Ok(PayloadData {
        signal_type: payload[0],
        timestamp: u64::from_be_bytes(payload[1..9].try_into()?),
        asset_id,
        coherence: u64::from_be_bytes(payload[41..49].try_into()?),
        confidence: u64::from_be_bytes(payload[49..57].try_into()?),
        manipulation_flags: u32::from_be_bytes(payload[57..61].try_into()?),
        limiting_layer: payload[61],
        coherence_gap: u64::from_be_bytes(payload[62..70].try_into()?),
        trend: payload[70],
        eta_recovery: u64::from_be_bytes(payload[71..79].try_into()?),
    })
}

fn payload_chunks(payload: &[u8]) -> Result<[H256; 8]> {
    if payload.len() < trion::SIGNAL_ENCODING_BYTES {
        anyhow::bail!("payload must be 256 bytes");
    }
    let mut chunks = [H256::zero(); 8];
    for i in 0..8 {
        let start = i * 32;
        let end = start + 32;
        chunks[i] = H256::from_slice(&payload[start..end]);
    }
    Ok(chunks)
}
