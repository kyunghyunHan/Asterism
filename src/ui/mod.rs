use std::collections::{BTreeMap, VecDeque};
pub mod chart;
use iced::Point;
pub mod buttons;
pub mod trading;
use crate::SignalScoring;
pub mod infos;
//chart 구조체
// ui/mod.rs
pub struct Chart {
    pub candlesticks: VecDeque<(u64, Candlestick)>,
    pub max_data_points: usize,
    pub state: ChartState,
    pub price_range: Option<(f32, f32)>,
    pub candle_type: CandleType,

    // 이동평균선 (기존 유지)
    pub show_ma5: bool,
    pub show_ma10: bool,
    pub show_ma20: bool,
    pub show_ma200: bool,
    pub ma5_values: BTreeMap<u64, f32>,
    pub ma10_values: BTreeMap<u64, f32>,
    pub ma20_values: BTreeMap<u64, f32>,
    pub ma200_values: BTreeMap<u64, f32>,

    // RSI (기존 유지)
    pub rsi_values: BTreeMap<u64, f32>,
    pub show_rsi: bool,

    // 점수 기반 신호만 새로 추가
    pub scored_signals_enabled: bool,
    pub buy_scored_signals: BTreeMap<u64, SignalScoring>,
    pub sell_scored_signals: BTreeMap<u64, SignalScoring>,
}

#[derive(Default, Debug)]
pub struct ChartState {
    pub offset: f32,
    pub dragging: bool,
    pub drag_start: Point,
    pub last_offset: f32,
    pub auto_scroll: bool,
    pub need_more_data: bool, // 추가
} // Candlestick 구조체 업데이트
#[derive(Debug, Clone)]
pub struct Candlestick {
    pub open: f32,
    pub close: f32,
    pub high: f32,
    pub low: f32,
    pub volume: f32, // 거래량 필드 추가
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum CandleType {
    Minute1,
    Minute3, // 2분봉을 3분봉으로 변경
    Day,
}
