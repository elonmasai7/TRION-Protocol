use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const SIGNAL_ENCODING_BYTES: usize = 256;
pub const SCORE_SCALE: f64 = 1_000_000.0;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Trend {
    Flat,
    Up,
    Down,
}

impl Trend {
    pub fn as_u8(self) -> u8 {
        match self {
            Trend::Flat => 0,
            Trend::Up => 1,
            Trend::Down => 2,
        }
    }

    pub fn from_u8(value: u8) -> Trend {
        match value {
            1 => Trend::Up,
            2 => Trend::Down,
            _ => Trend::Flat,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum SignalType {
    Signal,
    Silence,
}

impl SignalType {
    pub fn as_u8(self) -> u8 {
        match self {
            SignalType::Signal => 1,
            SignalType::Silence => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ManipulationType {
    WashTrading,
    CoordinatedPump,
    OracleAttackAttempt,
    SybilLiquidity,
    GovernanceCapture,
    MevExtractionSustained,
    FakeVolumeProtocol,
}

impl ManipulationType {
    pub fn flag(self) -> u32 {
        match self {
            ManipulationType::WashTrading => 1 << 0,
            ManipulationType::CoordinatedPump => 1 << 1,
            ManipulationType::OracleAttackAttempt => 1 << 2,
            ManipulationType::SybilLiquidity => 1 << 3,
            ManipulationType::GovernanceCapture => 1 << 4,
            ManipulationType::MevExtractionSustained => 1 << 5,
            ManipulationType::FakeVolumeProtocol => 1 << 6,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BehavioralMetrics {
    pub block_height: u64,
    pub timestamp: DateTime<Utc>,
    pub asset_id: [u8; 32],
    pub liquidity_ratio: f64,
    pub tx_entropy: f64,
    pub wallet_integrity: f64,
    pub governance_stability: f64,
    pub cross_chain_flow: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LayerScore {
    pub score: f64,
    pub confidence: f64,
    pub trend: Trend,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManipulationAlert {
    pub asset_id: [u8; 32],
    pub timestamp: DateTime<Utc>,
    pub kind: ManipulationType,
    pub severity: f64,
    pub description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SilenceDetails {
    pub limiting_layer: u8,
    pub coherence_gap: f64,
    pub trend: Trend,
    pub eta_recovery_blocks: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Signal {
    pub signal_type: SignalType,
    pub timestamp: DateTime<Utc>,
    pub asset_id: [u8; 32],
    pub coherence_score: f64,
    pub confidence: f64,
    pub manipulation_flags: u32,
    pub silence: Option<SilenceDetails>,
}

pub fn clamp_unit(value: f64) -> f64 {
    if value.is_nan() {
        0.0
    } else if value < 0.0 {
        0.0
    } else if value > 1.0 {
        1.0
    } else {
        value
    }
}

pub fn to_fixed(value: f64) -> u64 {
    let clamped = clamp_unit(value);
    (clamped * SCORE_SCALE).round() as u64
}

pub fn from_fixed(value: u64) -> f64 {
    (value as f64) / SCORE_SCALE
}

pub fn encode_signal_256(signal: &Signal) -> [u8; SIGNAL_ENCODING_BYTES] {
    let mut buf = [0u8; SIGNAL_ENCODING_BYTES];
    buf[0] = signal.signal_type.as_u8();

    let ts = signal.timestamp.timestamp() as u64;
    buf[1..9].copy_from_slice(&ts.to_be_bytes());

    buf[9..41].copy_from_slice(&signal.asset_id);

    let coherence = to_fixed(signal.coherence_score);
    buf[41..49].copy_from_slice(&coherence.to_be_bytes());

    let confidence = to_fixed(signal.confidence);
    buf[49..57].copy_from_slice(&confidence.to_be_bytes());

    buf[57..61].copy_from_slice(&signal.manipulation_flags.to_be_bytes());

    if let Some(silence) = &signal.silence {
        buf[61] = silence.limiting_layer;
        let gap = to_fixed(silence.coherence_gap);
        buf[62..70].copy_from_slice(&gap.to_be_bytes());
        buf[70] = silence.trend.as_u8();
        buf[71..79].copy_from_slice(&silence.eta_recovery_blocks.to_be_bytes());
    }

    buf
}

pub fn decode_signal_256(data: &[u8]) -> Result<Signal> {
    if data.len() < SIGNAL_ENCODING_BYTES {
        anyhow::bail!("payload must be 256 bytes");
    }
    let signal_type = match data[0] {
        1 => SignalType::Signal,
        2 => SignalType::Silence,
        _ => anyhow::bail!("unknown signal type"),
    };

    let ts = u64::from_be_bytes(data[1..9].try_into()?);
    let timestamp = DateTime::<Utc>::from_timestamp(ts as i64, 0).ok_or_else(|| anyhow::anyhow!("invalid timestamp"))?;

    let mut asset_id = [0u8; 32];
    asset_id.copy_from_slice(&data[9..41]);

    let coherence = u64::from_be_bytes(data[41..49].try_into()?);
    let confidence = u64::from_be_bytes(data[49..57].try_into()?);
    let manipulation_flags = u32::from_be_bytes(data[57..61].try_into()?);

    let silence = if signal_type == SignalType::Silence {
        let limiting_layer = data[61];
        let gap = u64::from_be_bytes(data[62..70].try_into()?);
        let trend = Trend::from_u8(data[70]);
        let eta = u64::from_be_bytes(data[71..79].try_into()?);
        Some(SilenceDetails {
            limiting_layer,
            coherence_gap: from_fixed(gap),
            trend,
            eta_recovery_blocks: eta,
        })
    } else {
        None
    };

    Ok(Signal {
        signal_type,
        timestamp,
        asset_id,
        coherence_score: from_fixed(coherence),
        confidence: from_fixed(confidence),
        manipulation_flags,
        silence,
    })
}

pub fn asset_id_from_hex(input: &str) -> Result<[u8; 32]> {
    let trimmed = input.trim_start_matches("0x");
    if trimmed.len() != 64 {
        anyhow::bail!("asset id must be 32 bytes hex");
    }
    let mut out = [0u8; 32];
    for i in 0..32 {
        let idx = i * 2;
        out[i] = u8::from_str_radix(&trimmed[idx..idx + 2], 16)?;
    }
    Ok(out)
}
