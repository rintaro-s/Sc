//! CRS Bridge
//!
//! Blender / Unity / Three.js などから送られる
//! CAS (CR-Absolute-Space) 形式の Transform を ATPPacket へ変換する。

use serde::{Deserialize, Serialize};

use crate::types::{ATPPacket, BridgeType, EntityType, Quaternionf, Vec3f};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CRSVec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CRSQuaternion {
    /// [x, y, z, w]
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceFrame {
    pub source: Option<String>,
    pub handedness: Option<String>,
    pub up_axis: Option<String>,
    pub forward_axis: Option<String>,
    pub unit_scale_to_meter: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbsoluteTransform {
    pub entity_id: u64,
    pub timestamp_us: u64,
    pub position_m: CRSVec3,
    pub rotation: CRSQuaternion,
    pub scale: Option<CRSVec3>,
    pub linear_velocity_mps: Option<CRSVec3>,
    pub angular_velocity_rps: Option<CRSVec3>,
    pub source_frame: Option<SourceFrame>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbsoluteTransformBatch {
    pub frame_id: Option<u64>,
    pub timestamp_us: Option<u64>,
    pub transforms: Vec<AbsoluteTransform>,
}

pub struct CRSBridge;

impl CRSBridge {
    /// JSON バッチを ATPPacket 群へ変換
    pub fn parse_json_batch(input: &str) -> Result<Vec<ATPPacket>, serde_json::Error> {
        let batch: AbsoluteTransformBatch = serde_json::from_str(input)?;
        Ok(Self::batch_to_atp(&batch))
    }

    /// CAS バッチを ATPPacket 群へ変換
    pub fn batch_to_atp(batch: &AbsoluteTransformBatch) -> Vec<ATPPacket> {
        let mut out = Vec::with_capacity(batch.transforms.len());
        for (i, t) in batch.transforms.iter().enumerate() {
            let vel = t
                .linear_velocity_mps
                .as_ref()
                .map(|v| Vec3f::new(v.x, v.y, v.z))
                .unwrap_or(Vec3f::ZERO);

            let ang = t
                .angular_velocity_rps
                .as_ref()
                .map(|v| Vec3f::new(v.x, v.y, v.z))
                .unwrap_or(Vec3f::ZERO);

            out.push(ATPPacket {
                entity_id: t.entity_id,
                timestamp_us: if t.timestamp_us == 0 {
                    ATPPacket::now_us()
                } else {
                    t.timestamp_us
                },
                sequence: i as u32,
                position: Vec3f::new(t.position_m.x, t.position_m.y, t.position_m.z),
                velocity: vel,
                acceleration: Vec3f::ZERO,
                // CAS は [x,y,z,w]。ATP は {w,x,y,z} へ詰め替え。
                orientation: Quaternionf {
                    w: t.rotation.w,
                    x: t.rotation.x,
                    y: t.rotation.y,
                    z: t.rotation.z,
                },
                angular_velocity: ang,
                imu_confidence: 1.0,
                entity_type: EntityType::ARObject,
                source_bridge: BridgeType::Direct,
            });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_json_batch_to_atp() {
        let json = r#"{
            "frame_id": 1,
            "timestamp_us": 100,
            "transforms": [
              {
                "entity_id": 42,
                "timestamp_us": 777,
                "position_m": {"x":1.0,"y":2.0,"z":3.0},
                "rotation": {"x":0.0,"y":0.0,"z":0.0,"w":1.0}
              }
            ]
        }"#;

        let packets = CRSBridge::parse_json_batch(json).expect("json parse should succeed");
        assert_eq!(packets.len(), 1);
        assert_eq!(packets[0].entity_id, 42);
        assert_eq!(packets[0].position.x, 1.0);
        assert_eq!(packets[0].orientation.w, 1.0);
    }
}
