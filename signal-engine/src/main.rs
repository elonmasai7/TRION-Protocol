use anyhow::Result;
use async_nats::Message;
use chrono::Utc;
use clap::Parser;
use serde_json::from_slice;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::{select, task};
use tracing::{info, warn};
use trion::{BehavioralMetrics, LayerScore, ManipulationAlert, Signal, SignalType, SilenceDetails, Trend};
use futures_util::StreamExt;

#[derive(Parser, Debug)]
#[command(name = "trion-signal-engine")]
struct Config {
    #[arg(long, env = "TRION_NATS_URL", default_value = "nats://nats:4222")]
    nats_url: String,

    #[arg(long, env = "TRION_SIGNAL_SUBJECT", default_value = "trion.signals")]
    signal_subject: String,

    #[arg(long, env = "TRION_ALERT_SUBJECT", default_value = "behavior.alerts")]
    alert_subject: String,

    #[arg(long, env = "TRION_COHERENCE_THRESHOLD", default_value_t = 0.6)]
    coherence_threshold: f64,

    #[arg(long, env = "TRION_M_MOAT", default_value_t = 0.00005)]
    m_moat: f64,

    #[arg(long, env = "TRION_LAYER_WEIGHTS", default_value = "0.25,0.2,0.2,0.2,0.15")]
    layer_weights: String,
}

#[derive(Default)]
struct State {
    last_metrics: HashMap<String, BehavioralMetrics>,
    last_coherence: HashMap<String, f64>,
    alerts: HashMap<String, Vec<ManipulationAlert>>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    let config = Config::parse();
    let weights = parse_weights(&config.layer_weights)?;

    let client = async_nats::connect(config.nats_url.clone()).await?;
    let mut metrics_sub = client.subscribe("behavior.metrics".into()).await?;
    let mut alert_sub = client.subscribe(config.alert_subject.clone().into()).await?;

    let state = Arc::new(Mutex::new(State::default()));
    let alert_state = Arc::clone(&state);

    task::spawn(async move {
        while let Some(message) = alert_sub.next().await {
            if let Ok(alert) = from_slice::<ManipulationAlert>(&message.payload) {
                let mut guard = alert_state.lock().await;
                let key = asset_key(&alert.asset_id);
                guard.alerts.entry(key).or_default().push(alert);
            }
        }
    });

    loop {
        select! {
            message = metrics_sub.next() => {
                if let Some(message) = message {
                    if let Ok(metrics) = from_slice::<BehavioralMetrics>(&message.payload) {
                        if let Err(err) = handle_metrics(&client, &config, &weights, &state, metrics).await {
                            warn!(error = %err, "failed to handle metrics");
                        }
                    }
                }
            }
        }
    }
}

async fn handle_metrics(
    client: &async_nats::Client,
    config: &Config,
    weights: &[f64],
    state: &Arc<Mutex<State>>,
    metrics: BehavioralMetrics,
) -> Result<()> {
    let mut guard = state.lock().await;
    let key = asset_key(&metrics.asset_id);
    let previous = guard.last_metrics.get(&key).cloned();
    let prev_coherence = guard.last_coherence.get(&key).cloned().unwrap_or(0.5);

    let layers = compute_layers(&metrics, previous.as_ref());
    let mut coherence = weighted_coherence(&layers, weights);

    let alerts = guard.alerts.get(&key).cloned().unwrap_or_default();
    let (flags, penalty) = apply_alert_penalty(&alerts);
    coherence = (coherence - penalty).clamp(0.0, 1.0);

    let confidence = layers.iter().map(|layer| layer.confidence).sum::<f64>() / layers.len() as f64;
    let overall_trend = trend_from_delta(prev_coherence, coherence);

    let signal = if coherence >= config.coherence_threshold {
        Signal {
            signal_type: SignalType::Signal,
            timestamp: Utc::now(),
            asset_id: metrics.asset_id,
            coherence_score: coherence,
            confidence,
            manipulation_flags: flags,
            silence: None,
        }
    } else {
        let (limiting_layer, min_score) = find_limiting_layer(&layers);
        let gap = (config.coherence_threshold - coherence).clamp(0.0, 1.0);
        let eta_blocks = estimate_recovery_blocks(gap, overall_trend);
        Signal {
            signal_type: SignalType::Silence,
            timestamp: Utc::now(),
            asset_id: metrics.asset_id,
            coherence_score: coherence,
            confidence,
            manipulation_flags: flags,
            silence: Some(SilenceDetails {
                limiting_layer,
                coherence_gap: gap,
                trend: overall_trend,
                eta_recovery_blocks: eta_blocks,
            }),
        }
    };

    let signal_value = signal_output_value(coherence, metrics.block_height, config.m_moat);
    info!(asset = key, coherence, signal_value, "signal computed");

    let payload = serde_json::to_vec(&signal)?;
    client
        .publish(config.signal_subject.clone().into(), payload.into())
        .await?;

    guard.last_metrics.insert(key.clone(), metrics);
    guard.last_coherence.insert(key, coherence);
    Ok(())
}

fn compute_layers(metrics: &BehavioralMetrics, previous: Option<&BehavioralMetrics>) -> Vec<LayerScore> {
    let mut layers = Vec::with_capacity(5);
    layers.push(layer(metrics.liquidity_ratio, previous.map(|p| p.liquidity_ratio)));
    layers.push(layer(metrics.tx_entropy, previous.map(|p| p.tx_entropy)));
    layers.push(layer(metrics.wallet_integrity, previous.map(|p| p.wallet_integrity)));
    layers.push(layer(metrics.governance_stability, previous.map(|p| p.governance_stability)));
    layers.push(layer(metrics.cross_chain_flow, previous.map(|p| p.cross_chain_flow)));
    layers
}

fn layer(current: f64, previous: Option<f64>) -> LayerScore {
    let score = current.clamp(0.0, 1.0);
    let confidence = 0.5 + (score - 0.5).abs();
    let trend = match previous {
        Some(prev) => trend_from_delta(prev, score),
        None => Trend::Flat,
    };
    LayerScore { score, confidence, trend }
}

fn weighted_coherence(layers: &[LayerScore], weights: &[f64]) -> f64 {
    let mut weighted_sum = 0.0;
    let mut weight_total = 0.0;
    for (idx, layer) in layers.iter().enumerate() {
        let weight = weights.get(idx).copied().unwrap_or(0.2);
        weighted_sum += layer.score * weight;
        weight_total += weight;
    }
    if weight_total == 0.0 {
        0.0
    } else {
        weighted_sum / weight_total
    }
}

fn apply_alert_penalty(alerts: &[ManipulationAlert]) -> (u32, f64) {
    let mut flags = 0u32;
    let mut penalty = 0.0;
    for alert in alerts {
        flags |= alert.kind.flag();
        penalty += (alert.severity * 0.08).min(0.15);
    }
    (flags, penalty.clamp(0.0, 0.4))
}

fn find_limiting_layer(layers: &[LayerScore]) -> (u8, f64) {
    let mut min_score = 1.0;
    let mut min_index = 1;
    for (idx, layer) in layers.iter().enumerate() {
        if layer.score < min_score {
            min_score = layer.score;
            min_index = idx + 1;
        }
    }
    (min_index as u8, min_score)
}

fn estimate_recovery_blocks(gap: f64, trend: Trend) -> u64 {
    let base = (gap / 0.01).round() as u64;
    match trend {
        Trend::Up => base.saturating_add(20).max(20),
        Trend::Down => base.saturating_add(200).max(200),
        Trend::Flat => base.saturating_add(100).max(100),
    }
}

fn signal_output_value(coherence: f64, block_height: u64, m_moat: f64) -> f64 {
    let t = block_height as f64;
    let s = coherence;
    if coherence >= 0.0 {
        s * (m_moat * t).exp()
    } else {
        0.0
    }
}

fn trend_from_delta(previous: f64, current: f64) -> Trend {
    let delta = current - previous;
    if delta > 0.02 {
        Trend::Up
    } else if delta < -0.02 {
        Trend::Down
    } else {
        Trend::Flat
    }
}

fn asset_key(asset_id: &[u8; 32]) -> String {
    let mut out = String::from("0x");
    for byte in asset_id {
        out.push_str(&format!("{:02x}", byte));
    }
    out
}

fn parse_weights(input: &str) -> Result<Vec<f64>> {
    let mut weights = Vec::new();
    for item in input.split(',') {
        let value: f64 = item.trim().parse()?;
        weights.push(value);
    }
    if weights.len() != 5 {
        anyhow::bail!("expected 5 layer weights");
    }
    Ok(weights)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn emits_silence_when_below_threshold() {
        let metrics = BehavioralMetrics {
            block_height: 10,
            timestamp: Utc::now(),
            asset_id: [1u8; 32],
            liquidity_ratio: 0.2,
            tx_entropy: 0.2,
            wallet_integrity: 0.2,
            governance_stability: 0.2,
            cross_chain_flow: 0.2,
        };
        let layers = compute_layers(&metrics, None);
        let coherence = weighted_coherence(&layers, &[0.2, 0.2, 0.2, 0.2, 0.2]);
        assert!(coherence < 0.6);
        let (limiting_layer, _) = find_limiting_layer(&layers);
        assert_eq!(limiting_layer, 1);
    }

    #[test]
    fn applies_manipulation_penalty() {
        let alert = ManipulationAlert {
            asset_id: [2u8; 32],
            timestamp: Utc::now(),
            kind: trion::ManipulationType::OracleAttackAttempt,
            severity: 0.9,
            description: "test".to_string(),
        };
        let (flags, penalty) = apply_alert_penalty(&[alert]);
        assert_ne!(flags, 0);
        assert!(penalty > 0.0);
    }
}
