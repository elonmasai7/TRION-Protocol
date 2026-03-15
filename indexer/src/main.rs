use anyhow::Result;
use async_nats::Client;
use chrono::Utc;
use clap::Parser;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};
use trion::BehavioralMetrics;

#[derive(Parser, Debug)]
#[command(name = "trion-indexer")]
struct Config {
    #[arg(long, env = "TRION_NATS_URL", default_value = "nats://nats:4222")]
    nats_url: String,

    #[arg(long, env = "TRION_ASSETS", default_value = "0x0000000000000000000000000000000000000000000000000000000000000001")]
    assets: String,

    #[arg(long, env = "TRION_BLOCK_START", default_value_t = 1)]
    block_start: u64,

    #[arg(long, env = "TRION_BLOCK_INTERVAL_MS", default_value_t = 1500)]
    block_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ParsedTransaction {
    from: String,
    to: String,
    value: f64,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    let config = Config::parse();
    let client = async_nats::connect(config.nats_url.clone()).await?;

    let asset_ids = parse_assets(&config.assets)?;
    let mut block_height = config.block_start;

    loop {
        let block_data = ingest_block(block_height).await;
        let transactions = parse_transactions(&block_data);
        let wallet_graph = build_wallet_graph(&transactions);
        let liquidity_flow = track_liquidity_flow(&transactions);
        let mev_signal = detect_mev_pattern(&transactions);
        let governance = monitor_governance_activity(&transactions);

        for asset_id in &asset_ids {
            let metrics = build_metrics(
                block_height,
                *asset_id,
                liquidity_flow,
                mev_signal,
                governance,
                wallet_graph.integrity_score,
            );
            publish_metrics(&client, &metrics).await?;
        }

        block_height += 1;
        sleep(Duration::from_millis(config.block_interval_ms)).await;
    }
}

async fn ingest_block(block_height: u64) -> String {
    info!(block_height, "ingesting block");
    format!("block-{}", block_height)
}

fn parse_transactions(_block: &str) -> Vec<ParsedTransaction> {
    info!("parsing transactions");
    let mut rng = rand::thread_rng();
    let mut txs = Vec::new();
    for _ in 0..20 {
        txs.push(ParsedTransaction {
            from: format!("0x{:x}", rng.gen::<u64>()),
            to: format!("0x{:x}", rng.gen::<u64>()),
            value: rng.gen_range(0.0..1000.0),
        });
    }
    txs
}

struct WalletGraphSummary {
    integrity_score: f64,
}

fn build_wallet_graph(_txs: &[ParsedTransaction]) -> WalletGraphSummary {
    info!("building wallet graph");
    WalletGraphSummary { integrity_score: rand::thread_rng().gen_range(0.4..0.95) }
}

fn track_liquidity_flow(_txs: &[ParsedTransaction]) -> f64 {
    info!("tracking liquidity flow");
    rand::thread_rng().gen_range(0.2..0.95)
}

fn detect_mev_pattern(_txs: &[ParsedTransaction]) -> f64 {
    info!("detecting MEV patterns");
    rand::thread_rng().gen_range(0.0..1.0)
}

fn monitor_governance_activity(_txs: &[ParsedTransaction]) -> f64 {
    info!("monitoring governance activity");
    rand::thread_rng().gen_range(0.4..0.98)
}

fn build_metrics(
    block_height: u64,
    asset_id: [u8; 32],
    liquidity_flow: f64,
    mev_signal: f64,
    governance: f64,
    wallet_integrity: f64,
) -> BehavioralMetrics {
    let mut rng = rand::thread_rng();
    let tx_entropy = 1.0 - mev_signal * 0.4 + rng.gen_range(-0.1..0.1);
    let cross_chain_flow = rng.gen_range(0.2..0.9);

    BehavioralMetrics {
        block_height,
        timestamp: Utc::now(),
        asset_id,
        liquidity_ratio: liquidity_flow,
        tx_entropy: tx_entropy.clamp(0.0, 1.0),
        wallet_integrity,
        governance_stability: governance,
        cross_chain_flow,
    }
}

async fn publish_metrics(client: &Client, metrics: &BehavioralMetrics) -> Result<()> {
    let payload = serde_json::to_vec(metrics)?;
    client.publish("behavior.metrics".into(), payload.into()).await?;
    Ok(())
}

fn parse_assets(input: &str) -> Result<Vec<[u8; 32]>> {
    let mut assets = Vec::new();
    for item in input.split(',') {
        match trion::asset_id_from_hex(item) {
            Ok(id) => assets.push(id),
            Err(err) => warn!(asset = item, error = %err, "invalid asset id"),
        }
    }
    if assets.is_empty() {
        anyhow::bail!("no valid assets configured");
    }
    Ok(assets)
}
