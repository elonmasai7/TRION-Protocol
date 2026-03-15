use anyhow::Result;
use async_nats::Message;
use chrono::Utc;
use clap::Parser;
use serde_json::from_slice;
use tokio::select;
use tracing::info;
use futures_util::StreamExt;
use trion::{BehavioralMetrics, ManipulationAlert, ManipulationType};

#[derive(Parser, Debug)]
#[command(name = "trion-manipulation-detector")]
struct Config {
    #[arg(long, env = "TRION_NATS_URL", default_value = "nats://nats:4222")]
    nats_url: String,

    #[arg(long, env = "TRION_ALERT_SUBJECT", default_value = "behavior.alerts")]
    alert_subject: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    let config = Config::parse();
    let client = async_nats::connect(config.nats_url.clone()).await?;
    let mut sub = client.subscribe("behavior.metrics".into()).await?;

    loop {
        select! {
            message = sub.next() => {
                if let Some(message) = message {
                    if let Some(alert) = handle_metric(message).await? {
                        let payload = serde_json::to_vec(&alert)?;
                        client.publish(config.alert_subject.clone().into(), payload.into()).await?;
                    }
                }
            }
        }
    }
}

async fn handle_metric(message: Message) -> Result<Option<ManipulationAlert>> {
    let metrics: BehavioralMetrics = from_slice(&message.payload)?;
    let mut alert: Option<ManipulationAlert> = None;

    if metrics.tx_entropy < 0.25 && metrics.liquidity_ratio > 0.8 {
        alert = Some(build_alert(
            metrics.asset_id,
            ManipulationType::WashTrading,
            0.7,
            "low entropy with high liquidity concentration",
        ));
    } else if metrics.cross_chain_flow > 0.85 && metrics.wallet_integrity < 0.4 {
        alert = Some(build_alert(
            metrics.asset_id,
            ManipulationType::SybilLiquidity,
            0.65,
            "cross-chain spike with weak wallet graph",
        ));
    } else if metrics.governance_stability < 0.35 {
        alert = Some(build_alert(
            metrics.asset_id,
            ManipulationType::GovernanceCapture,
            0.8,
            "governance instability detected",
        ));
    } else if metrics.tx_entropy < 0.2 && metrics.cross_chain_flow > 0.75 {
        alert = Some(build_alert(
            metrics.asset_id,
            ManipulationType::CoordinatedPump,
            0.6,
            "entropy collapse with flow surge",
        ));
    }

    if let Some(ref alert) = alert {
        info!(kind = ?alert.kind, severity = alert.severity, "manipulation alert");
    }

    Ok(alert)
}

fn build_alert(asset_id: [u8; 32], kind: ManipulationType, severity: f64, description: &str) -> ManipulationAlert {
    ManipulationAlert {
        asset_id,
        timestamp: Utc::now(),
        kind,
        severity,
        description: description.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn detects_wash_trading() {
        let metrics = BehavioralMetrics {
            block_height: 1,
            timestamp: Utc::now(),
            asset_id: [1u8; 32],
            liquidity_ratio: 0.9,
            tx_entropy: 0.1,
            wallet_integrity: 0.8,
            governance_stability: 0.7,
            cross_chain_flow: 0.2,
        };
        let payload = serde_json::to_vec(&metrics).unwrap();
        let message = Message {
            subject: "behavior.metrics".into(),
            reply: None,
            payload: payload.into(),
            headers: None,
        };

        let alert = handle_metric(message).await.unwrap();
        assert!(alert.is_some());
        assert_eq!(alert.unwrap().kind, ManipulationType::WashTrading);
    }

    #[tokio::test]
    async fn detects_governance_capture() {
        let metrics = BehavioralMetrics {
            block_height: 1,
            timestamp: Utc::now(),
            asset_id: [9u8; 32],
            liquidity_ratio: 0.5,
            tx_entropy: 0.6,
            wallet_integrity: 0.7,
            governance_stability: 0.2,
            cross_chain_flow: 0.4,
        };
        let payload = serde_json::to_vec(&metrics).unwrap();
        let message = Message {
            subject: "behavior.metrics".into(),
            reply: None,
            payload: payload.into(),
            headers: None,
        };

        let alert = handle_metric(message).await.unwrap();
        assert!(alert.is_some());
        assert_eq!(alert.unwrap().kind, ManipulationType::GovernanceCapture);
    }
}
