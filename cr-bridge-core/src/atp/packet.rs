//! ATP パケットセッション管理
//!
//! エンティティごとのパケット受信履歴・シーケンス番号管理

use crate::types::ATPPacket;
use std::collections::VecDeque;

/// エンティティごとの ATP セッション
pub struct ATPSession {
    pub entity_id: u64,
    /// 受信済みシーケンス番号の最大値
    pub last_seq: u32,
    /// パケット受信履歴（直近N件）
    pub packet_history: VecDeque<ATPPacket>,
    /// 受信したパケット総数
    pub total_received: u64,
    /// 欠落したパケット総数
    pub total_lost: u64,
    /// 最後のパケット受信時刻 (us)
    pub last_received_us: u64,
}

impl ATPSession {
    pub fn new(entity_id: u64) -> Self {
        Self {
            entity_id,
            last_seq: 0,
            packet_history: VecDeque::with_capacity(64),
            total_received: 0,
            total_lost: 0,
            last_received_us: 0,
        }
    }

    /// パケットを受信して履歴に追加
    ///
    /// ## 戻り値
    /// - `(received, lost_count)` — 今回受信 + 欠落検知数
    pub fn receive_packet(&mut self, packet: ATPPacket) -> (bool, u32) {
        let seq = packet.sequence;
        let now_us = packet.timestamp_us;

        // シーケンス番号から欠落を検知
        let lost = if self.total_received == 0 {
            0
        } else {
            let expected = self.last_seq.wrapping_add(1);
            if seq > expected {
                seq - expected // 欠落パケット数
            } else {
                0
            }
        };

        self.total_lost += lost as u64;
        self.total_received += 1;
        self.last_seq = seq;
        self.last_received_us = now_us;

        // 履歴は最大64件まで保持
        if self.packet_history.len() >= 64 {
            self.packet_history.pop_front();
        }
        self.packet_history.push_back(packet);

        (true, lost)
    }

    /// パケットロス率を返す
    pub fn packet_loss_rate(&self) -> f32 {
        let total = self.total_received + self.total_lost;
        if total == 0 {
            0.0
        } else {
            self.total_lost as f32 / total as f32
        }
    }

    /// 最後に受信したパケットを返す
    pub fn last_packet(&self) -> Option<&ATPPacket> {
        self.packet_history.back()
    }
}
