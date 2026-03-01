//! SeqLock (Sequence Lock) 実装
//!
//! 単一ライター・複数リーダー向けの lockfree な排他制御。
//! mutex を使わずに書き込み中のデータを読み取れる。
//!
//! ## 仕組み
//! 1. 書き込み前にシーケンス番号を奇数にする（書き込み中フラグ）
//! 2. 書き込み完了後にシーケンス番号を偶数にする
//! 3. 読み取り側は読み前後でシーケンス番号が同じ偶数かを確認
//!    → 書き込み中に読んだ場合は再試行
//!
//! SIMDコアから書き込み、描画コアから読み出す際に使用。

use std::sync::atomic::{AtomicU64, Ordering};

/// SeqLock
///
/// `T` を保護する SeqLock。`T` は `Copy` である必要がある。
pub struct SeqLock<T: Copy> {
    /// シーケンス番号（奇数 = 書き込み中）
    seq: AtomicU64,
    /// 保護するデータ
    ///
    /// Safety: seq によって保護される。
    data: std::cell::UnsafeCell<T>,
}

// Safety: SeqLock は内部でアトミック操作と seq による保護を行う
unsafe impl<T: Copy + Send> Send for SeqLock<T> {}
unsafe impl<T: Copy + Send + Sync> Sync for SeqLock<T> {}

impl<T: Copy + Default> SeqLock<T> {
    pub fn new(initial: T) -> Self {
        Self {
            seq: AtomicU64::new(0),
            data: std::cell::UnsafeCell::new(initial),
        }
    }

    /// データを書き込む（単一ライターのみ）
    ///
    /// Safety: この関数を同時に複数のスレッドから呼んではならない
    pub fn write(&self, value: T) {
        // シーケンス番号を奇数にして「書き込み中」を通知
        let seq = self.seq.load(Ordering::Relaxed);
        self.seq.store(seq + 1, Ordering::Release);

        // 書き込み
        unsafe {
            *self.data.get() = value;
        }

        // シーケンス番号を偶数に戻して「書き込み完了」
        std::sync::atomic::fence(Ordering::Release);
        self.seq.store(seq + 2, Ordering::Release);
    }

    /// データを読み取る（複数リーダー可）
    ///
    /// 書き込み中の場合は再試行する（スピン）
    pub fn read(&self) -> T {
        loop {
            let seq1 = self.seq.load(Ordering::Acquire);

            // 奇数 = 書き込み中 → 待機
            if seq1 & 1 != 0 {
                std::hint::spin_loop();
                continue;
            }

            // データ読み取り
            let value = unsafe { *self.data.get() };

            std::sync::atomic::fence(Ordering::Acquire);
            let seq2 = self.seq.load(Ordering::Acquire);

            // 読み取り中に書き込みが入った場合は再試行
            if seq1 == seq2 {
                return value;
            }
        }
    }

    /// 現在のシーケンス番号を返す（デバッグ用）
    pub fn sequence(&self) -> u64 {
        self.seq.load(Ordering::Relaxed)
    }
}

/// SeqLock で保護された位置・速度・姿勢
///
/// SMSL エンティティエントリの高頻度更新フィールドを格納
#[derive(Debug, Clone, Copy, Default)]
#[repr(C, align(64))] // キャッシュライン境界に揃える
pub struct SeqLockedPose {
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub orientation: [f32; 4], // [w, x, y, z]
    pub timestamp_us: u64,
    pub _pad: [u8; 4],
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_seqlock_single_thread() {
        #[derive(Clone, Copy, Default)]
        struct Data {
            x: f32,
            y: f32,
        }

        let lock = SeqLock::new(Data { x: 0.0, y: 0.0 });
        lock.write(Data { x: 1.0, y: 2.0 });
        let d = lock.read();
        assert!((d.x - 1.0).abs() < 1e-6);
        assert!((d.y - 2.0).abs() < 1e-6);
    }

    #[test]
    fn test_seqlock_concurrent() {
        #[derive(Clone, Copy, Default)]
        struct Data {
            value: u64,
        }

        let lock = Arc::new(SeqLock::new(Data { value: 0 }));
        let lock_reader = Arc::clone(&lock);

        // リーダースレッドを起動
        let reader = thread::spawn(move || {
            for _ in 0..1000 {
                let _d = lock_reader.read();
            }
        });

        // ライタースレッドで書き込み
        for i in 0..100u64 {
            lock.write(Data { value: i });
        }

        reader.join().unwrap();
        let final_val = lock.read();
        assert_eq!(final_val.value, 99);
    }
}
