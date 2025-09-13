pub mod knn;
use std::collections::VecDeque;

// KNN 예측기 최적화 버전
#[derive(Debug, Clone)]
pub struct OptimizedKNNPredictor {
    pub k: usize,
    pub window_size: usize,
    pub features_buffer: VecDeque<Vec<f32>>,
    pub labels_buffer: VecDeque<bool>,
}

// models.rs에 추가
use crate::Candlestick;
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct SignalScoring {
    pub bullish_engulfing: f32, // 0-100
    pub bearish_engulfing: f32, // 0-100
    pub morning_star: f32,      // 0-100
    pub evening_star: f32,      // 0-100
    pub total_score: f32,       // 0-100
}

impl SignalScoring {
    pub fn new() -> Self {
        Self {
            bullish_engulfing: 0.0,
            bearish_engulfing: 0.0,
            morning_star: 0.0,
            evening_star: 0.0,
            total_score: 0.0,
        }
    }
}

pub struct CandlestickPatterns;

impl CandlestickPatterns {
    // 상승 포용선 패턴 감지
    pub fn detect_bullish_engulfing(data: &[(&u64, &Candlestick)], current_idx: usize) -> f32 {
        if current_idx == 0 {
            return 0.0;
        }

        let prev_candle = data[current_idx - 1].1;
        let curr_candle = data[current_idx].1;

        // 이전 캔들: 하락 (빨간색)
        let prev_bearish = prev_candle.close < prev_candle.open;
        let prev_body = (prev_candle.open - prev_candle.close).abs();

        // 현재 캔들: 상승 (초록색)
        let curr_bullish = curr_candle.close > curr_candle.open;
        let curr_body = curr_candle.close - curr_candle.open;

        // 포용 조건들
        let engulfs_open = curr_candle.open <= prev_candle.close;
        let engulfs_close = curr_candle.close >= prev_candle.open;
        let size_ratio = curr_body / prev_body.max(0.0001);

        if prev_bearish && curr_bullish && engulfs_open && engulfs_close {
            // 포용 강도에 따른 점수 (최대 25점)
            let base_score = 15.0;
            let size_bonus = (size_ratio - 1.0).min(1.0) * 10.0;
            (base_score + size_bonus).min(25.0)
        } else {
            0.0
        }
    }

    // 하락 포용선 패턴 감지
    pub fn detect_bearish_engulfing(data: &[(&u64, &Candlestick)], current_idx: usize) -> f32 {
        if current_idx == 0 {
            return 0.0;
        }

        let prev_candle = data[current_idx - 1].1;
        let curr_candle = data[current_idx].1;

        // 이전 캔들: 상승 (초록색)
        let prev_bullish = prev_candle.close > prev_candle.open;
        let prev_body = prev_candle.close - prev_candle.open;

        // 현재 캔들: 하락 (빨간색)
        let curr_bearish = curr_candle.close < curr_candle.open;
        let curr_body = (curr_candle.open - curr_candle.close).abs();

        // 포용 조건들
        let engulfs_open = curr_candle.open >= prev_candle.close;
        let engulfs_close = curr_candle.close <= prev_candle.open;
        let size_ratio = curr_body / prev_body.max(0.0001);

        if prev_bullish && curr_bearish && engulfs_open && engulfs_close {
            let base_score = 15.0;
            let size_bonus = (size_ratio - 1.0).min(1.0) * 10.0;
            (base_score + size_bonus).min(25.0)
        } else {
            0.0
        }
    }

    // 샛별 패턴 감지 (3캔들 패턴)
    pub fn detect_morning_star(data: &[(&u64, &Candlestick)], current_idx: usize) -> f32 {
        if current_idx < 2 {
            return 0.0;
        }

        let first = data[current_idx - 2].1; // 큰 하락 캔들
        let middle = data[current_idx - 1].1; // 작은 캔들 (십자선)
        let last = data[current_idx].1; // 큰 상승 캔들

        // 첫 번째: 강한 하락 캔들
        let first_bearish = first.close < first.open;
        let first_body = (first.open - first.close).abs();
        let first_strong = first_body > (first.high - first.low) * 0.6;

        // 두 번째: 작은 캔들 (갭 다운)
        let middle_small = (middle.close - middle.open).abs() < first_body * 0.3;
        let gap_down = middle.high < first.close;

        // 세 번째: 강한 상승 캔들 (갭 업)
        let last_bullish = last.close > last.open;
        let last_body = last.close - last.open;
        let last_strong = last_body > (last.high - last.low) * 0.6;
        let gap_up = last.low > middle.high;

        // 상승 포용 확인
        let recovery = last.close > (first.open + first.close) / 2.0;

        if first_bearish
            && first_strong
            && middle_small
            && gap_down
            && last_bullish
            && last_strong
            && gap_up
            && recovery
        {
            let strength = (last_body / first_body).min(1.5);
            (15.0 + strength * 10.0).min(25.0)
        } else {
            0.0
        }
    }

    // 저녁별 패턴 감지 (3캔들 패턴)
    pub fn detect_evening_star(data: &[(&u64, &Candlestick)], current_idx: usize) -> f32 {
        if current_idx < 2 {
            return 0.0;
        }

        let first = data[current_idx - 2].1; // 큰 상승 캔들
        let middle = data[current_idx - 1].1; // 작은 캔들 (십자선)
        let last = data[current_idx].1; // 큰 하락 캔들

        // 첫 번째: 강한 상승 캔들
        let first_bullish = first.close > first.open;
        let first_body = first.close - first.open;
        let first_strong = first_body > (first.high - first.low) * 0.6;

        // 두 번째: 작은 캔들 (갭 업)
        let middle_small = (middle.close - middle.open).abs() < first_body * 0.3;
        let gap_up = middle.low > first.close;

        // 세 번째: 강한 하락 캔들 (갭 다운)
        let last_bearish = last.close < last.open;
        let last_body = (last.open - last.close).abs();
        let last_strong = last_body > (last.high - last.low) * 0.6;
        let gap_down = last.high < middle.low;

        // 하락 포용 확인
        let decline = last.close < (first.open + first.close) / 2.0;

        if first_bullish
            && first_strong
            && middle_small
            && gap_up
            && last_bearish
            && last_strong
            && gap_down
            && decline
        {
            let strength = (last_body / first_body).min(1.5);
            (15.0 + strength * 10.0).min(25.0)
        } else {
            0.0
        }
    }
}
