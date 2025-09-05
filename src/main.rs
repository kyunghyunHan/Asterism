#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(rustdoc::missing_crate_level_docs)]

use eframe::egui;
use egui_plot::{BoxElem, BoxPlot, BoxSpread, Line, Plot, PlotPoints};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use chrono::Utc;
use chrono::TimeZone;
fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1600.0, 900.0])
            .with_title("Crypto Trading Chart"),
        ..Default::default()
    };

    eframe::run_native(
        "Crypto Trading Chart",
        options,
        Box::new(|_cc| Ok(Box::<CryptoApp>::default())),
    )
}
async fn fetch_klines_historical(
    timeframe: &Timeframe,
    start_time: i64,  // Unix timestamp in milliseconds
    limit: u16,
) -> Result<Vec<CandleData>, Box<dyn std::error::Error>> {
    let url = format!(
        "https://fapi.binance.com/fapi/v1/klines?symbol=BTCUSDT&interval={}&startTime={}&limit={}",
        timeframe.to_api_string(),
        start_time,
        limit
    );

    let client = reqwest::Client::new();
    let response = client.get(&url).send().await?;

    if !response.status().is_success() {
        return Err(format!("API error: {}", response.status()).into());
    }

    let text = response.text().await?;
    let json: serde_json::Value = serde_json::from_str(&text)?;

    let mut candles = Vec::new();

    if let Some(array) = json.as_array() {
        for item in array {
            if let Some(kline_array) = item.as_array() {
                if kline_array.len() >= 11 {
                    let timestamp = kline_array[0].as_i64().unwrap_or(0) as f64;
                    let open = kline_array[1]
                        .as_str()
                        .unwrap_or("0")
                        .parse::<f64>()
                        .unwrap_or(0.0);
                    let high = kline_array[2]
                        .as_str()
                        .unwrap_or("0")
                        .parse::<f64>()
                        .unwrap_or(0.0);
                    let low = kline_array[3]
                        .as_str()
                        .unwrap_or("0")
                        .parse::<f64>()
                        .unwrap_or(0.0);
                    let close = kline_array[4]
                        .as_str()
                        .unwrap_or("0")
                        .parse::<f64>()
                        .unwrap_or(0.0);
                    let volume = kline_array[5]
                        .as_str()
                        .unwrap_or("0")
                        .parse::<f64>()
                        .unwrap_or(0.0);

                    if open > 0.0 && high > 0.0 && low > 0.0 && close > 0.0 {
                        candles.push(CandleData {
                            timestamp: timestamp / 1000.0,
                            open,
                            high,
                            low,
                            close,
                            volume,
                        });
                    }
                }
            }
        }
    }

    Ok(candles)
}
#[derive(Clone, Debug)]
struct CandleData {
    timestamp: f64,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
}

#[derive(Clone, PartialEq)]
enum ChartType {
    Line,
    Candlestick,
}

#[derive(Clone, PartialEq)]
enum Timeframe {
    M1,  // 1 minute
    M3,  // 3 minutes
    M5,  // 5 minutes
    M15, // 15 minutes
    M30, // 30 minutes
    H1,  // 1 hour
    H4,  // 4 hours
    H12, // 12 hours
    D1,  // Daily
    W1,  // Weekly
    MN1, // Monthly
}

impl Timeframe {
    fn to_api_string(&self) -> &'static str {
        match self {
            Timeframe::M1 => "1m",
            Timeframe::M3 => "3m",
            Timeframe::M5 => "5m",
            Timeframe::M15 => "15m",
            Timeframe::M30 => "30m",
            Timeframe::H1 => "1h",
            Timeframe::H4 => "4h",
            Timeframe::H12 => "12h",
            Timeframe::D1 => "1d",
            Timeframe::W1 => "1w",
            Timeframe::MN1 => "1M",
        }
    }

    fn to_display_string(&self) -> &'static str {
        match self {
            Timeframe::M1 => "1m",
            Timeframe::M3 => "3m",
            Timeframe::M5 => "5m",
            Timeframe::M15 => "15m",
            Timeframe::M30 => "30m",
            Timeframe::H1 => "1h",
            Timeframe::H4 => "4h",
            Timeframe::H12 => "12h",
            Timeframe::D1 => "1d",
            Timeframe::W1 => "1w",
            Timeframe::MN1 => "1M",
        }
    }

    fn get_window_size(&self) -> f64 {
        match self {
            Timeframe::M1 => 60.0 * 100.0,                      // 100 minutes
            Timeframe::M3 => 60.0 * 300.0,                      // 300 minutes
            Timeframe::M5 => 60.0 * 500.0,                      // 500 minutes
            Timeframe::M15 => 60.0 * 1500.0,                    // 1500 minutes
            Timeframe::M30 => 60.0 * 3000.0,                    // 3000 minutes
            Timeframe::H1 => 60.0 * 60.0 * 100.0,               // 100 hours
            Timeframe::H4 => 60.0 * 60.0 * 400.0,               // 400 hours
            Timeframe::H12 => 60.0 * 60.0 * 1200.0,             // 1200 hours
            Timeframe::D1 => 60.0 * 60.0 * 24.0 * 100.0,        // 100 days
            Timeframe::W1 => 60.0 * 60.0 * 24.0 * 7.0 * 50.0,   // 50 weeks
            Timeframe::MN1 => 60.0 * 60.0 * 24.0 * 30.0 * 12.0, // 12 months
        }
    }

    // Calculate candle interval in seconds
    fn get_candle_interval(&self) -> f64 {
        match self {
            Timeframe::M1 => 60.0,       // 1 minute
            Timeframe::M3 => 180.0,      // 3 minutes
            Timeframe::M5 => 300.0,      // 5 minutes
            Timeframe::M15 => 900.0,     // 15 minutes
            Timeframe::M30 => 1800.0,    // 30 minutes
            Timeframe::H1 => 3600.0,     // 1 hour
            Timeframe::H4 => 14400.0,    // 4 hours
            Timeframe::H12 => 43200.0,   // 12 hours
            Timeframe::D1 => 86400.0,    // 1 day
            Timeframe::W1 => 604800.0,   // 1 week
            Timeframe::MN1 => 2592000.0, // 1 month (30 days)
        }
    }
}
fn format_timestamp_to_date(timestamp: f64, timeframe: &Timeframe) -> String {
    let dt = Utc.timestamp_opt(timestamp as i64, 0).unwrap();
    
    match timeframe {
        Timeframe::M1 | Timeframe::M3 | Timeframe::M5 | Timeframe::M15 | Timeframe::M30 => {
            // 분 단위: 시:분 표시
            dt.format("%H:%M").to_string()
        },
        Timeframe::H1 | Timeframe::H4 | Timeframe::H12 => {
            // 시간 단위: 월-일 시:분 표시
            dt.format("%m-%d %H:%M").to_string()
        },
        Timeframe::D1 => {
            // 일 단위: 년-월-일 표시
            dt.format("%Y-%m-%d").to_string()
        },
        Timeframe::W1 | Timeframe::MN1 => {
            // 주/월 단위: 년-월 표시
            dt.format("%Y-%m").to_string()
        }
    }
}
#[derive(Clone, PartialEq)]
enum OrderType {
    Buy,
    Sell,
}

#[derive(Clone, PartialEq)]
enum OrderMode {
    Market,
    Limit,
}

struct TradingPanel {
    order_type: OrderType,
    order_mode: OrderMode,
    quantity: String,
    price: String,
    current_price: f64,
    balance_usdt: f64,
    balance_btc: f64,
}

impl Default for TradingPanel {
    fn default() -> Self {
        Self {
            order_type: OrderType::Buy,
            order_mode: OrderMode::Market,
            quantity: "0.001".to_string(),
            price: "0.0".to_string(),
            current_price: 0.0,
            balance_usdt: 10000.0, // Virtual balance
            balance_btc: 0.0,
        }
    }
}

struct CryptoApp {
    candle_data: Arc<Mutex<VecDeque<CandleData>>>,
    chart_type: ChartType,
    timeframe: Timeframe,
    candle_width: f64,
    is_loading: bool,
    runtime: Option<tokio::runtime::Runtime>,
    data_receiver: Option<mpsc::UnboundedReceiver<Vec<CandleData>>>,
    latest_timestamp: f64,
    view_window_start: f64,
    window_size: f64,
    is_dragging: bool,
    is_live_mode: bool,
    timeframe_changed: bool,
    trading_panel: TradingPanel,
    show_ma20: bool,
    show_bollinger: bool,
    show_macd: bool,
    show_rsi: bool,
    show_volume: bool,
    shared_drag_delta: f64,
    any_chart_dragged: bool,

    is_loading_historical: bool,
    earliest_timestamp: f64,
    historical_data_receiver: Option<mpsc::UnboundedReceiver<Vec<CandleData>>>,
}
impl CryptoApp {
    fn check_and_load_historical_data(&mut self) {
        // 현재 보기 윈도우의 시작점이 로드된 데이터의 시작점에 가까워졌는지 확인
        let buffer_time = self.window_size * 0.2; // 윈도우 크기의 20%를 버퍼로 사용
        
        if self.view_window_start <= (self.earliest_timestamp + buffer_time) 
            && !self.is_loading_historical 
            && self.earliest_timestamp > 0.0 {
            
            self.is_loading_historical = true;
            
            if let Some(rt) = &self.runtime {
                let (tx, rx) = mpsc::unbounded_channel();
                self.historical_data_receiver = Some(rx);
                
                let candle_data_clone = self.candle_data.clone();
                let timeframe_clone = self.timeframe.clone();
                let earliest_time = self.earliest_timestamp;
                
                rt.spawn(async move {
                    // 현재 가장 이른 시간에서 500개 더 과거로 가져오기
                    let start_time = (earliest_time as i64 - 
                        (500 * timeframe_clone.get_candle_interval() as i64)) * 1000;
                    
                    match fetch_klines_historical(&timeframe_clone, start_time, 500).await {
                        Ok(mut historical_candles) => {
                            // 역순으로 정렬하여 가장 오래된 것부터 앞에 추가
                            historical_candles.sort_by(|a, b| a.timestamp.partial_cmp(&b.timestamp).unwrap());
                            
                            if let Ok(mut data) = candle_data_clone.lock() {
                                // 기존 데이터 앞에 과거 데이터 추가
                                for candle in historical_candles.iter().rev() {
                                    if candle.timestamp < earliest_time {
                                        data.push_front(candle.clone());
                                    }
                                }
                                
                                // 메모리 사용량 제한 (최대 20000개)
                                while data.len() > 20000 {
                                    data.pop_back();
                                }
                            }
                            
                            let _ = tx.send(historical_candles);
                        }
                        Err(e) => {
                            eprintln!("Error fetching historical data: {}", e);
                            let _ = tx.send(Vec::new());
                        }
                    }
                });
            }
        }
    }
    fn handle_chart_drag(&mut self, plot_ui: &egui_plot::PlotUi) {
        if plot_ui.response().dragged() {
            let drag_delta = plot_ui.pointer_coordinate_drag_delta();
            if drag_delta.x.abs() > 0.1 {
                self.shared_drag_delta = drag_delta.x as f64;
                self.any_chart_dragged = true;
            }
        }
    }

    fn apply_shared_drag(&mut self) {
        if self.any_chart_dragged && self.shared_drag_delta.abs() > 0.1 {
            let right_margin = self.window_size * 0.1;
            let proposed_start = self.view_window_start - self.shared_drag_delta as f64;
            let proposed_end = proposed_start + self.window_size - right_margin;

            if proposed_end <= self.latest_timestamp && proposed_start >= 0.0 {
                self.view_window_start = proposed_start;
                self.is_live_mode = false;
            } else if proposed_end > self.latest_timestamp {
                self.view_window_start = self.latest_timestamp - self.window_size + right_margin;
                self.is_live_mode = true;
            } else if proposed_start < 0.0 {
                self.view_window_start = 0.0;
                self.is_live_mode = false;
            }

            self.is_dragging = true;
        }
    }
    fn calculate_visible_price_range(&self, filtered_data: &[CandleData]) -> (f64, f64) {
        if filtered_data.is_empty() {
            return (0.0, 100.0);
        }

        let mut min_price = f64::INFINITY;
        let mut max_price = f64::NEG_INFINITY;

        for candle in filtered_data {
            min_price = min_price.min(candle.low);
            max_price = max_price.max(candle.high);
        }

        let padding = (max_price - min_price) * 0.05;
        (min_price - padding, max_price + padding)
    }
}

impl Default for CryptoApp {
    fn default() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();

        let timeframe = Timeframe::M1;
        let window_size = timeframe.get_window_size();

        let app = Self {
            candle_data: Arc::new(Mutex::new(VecDeque::new())),
            chart_type: ChartType::Candlestick,
            timeframe,
            candle_width: 0.8,
            is_loading: true,
            runtime: Some(tokio::runtime::Runtime::new().unwrap()),
            data_receiver: Some(rx),
            latest_timestamp: 0.0,
            view_window_start: 0.0,
            window_size,
            is_dragging: false,
            is_live_mode: true,
            timeframe_changed: false,
            trading_panel: TradingPanel::default(),
            show_ma20: true,
            show_bollinger: false,
            show_macd: true,
            show_rsi: true,
            show_volume: true,
            shared_drag_delta: 0.0,
            any_chart_dragged: false,
            is_loading_historical: false,
            earliest_timestamp: 0.0,
            historical_data_receiver: None,
        };

        // Start fetching data
        if let Some(rt) = &app.runtime {
            let candle_data_clone = app.candle_data.clone();
            let timeframe_clone = app.timeframe.clone();
            rt.spawn(fetch_binance_data(tx, candle_data_clone, timeframe_clone));
        }

        app
    }
}

async fn fetch_binance_data(
    tx: mpsc::UnboundedSender<Vec<CandleData>>,
    candle_data: Arc<Mutex<VecDeque<CandleData>>>,
    timeframe: Timeframe,
) {
    loop {
        match fetch_klines_latest(&timeframe).await {
            Ok(candles) => {
                if let Ok(mut data) = candle_data.lock() {
                    if data.is_empty() {
                        data.extend(candles.iter().cloned());
                    } else {
                        let latest_existing_time = data.back().map(|d| d.timestamp).unwrap_or(0.0);

                        for new_candle in &candles {
                            if new_candle.timestamp > latest_existing_time {
                                data.push_back(new_candle.clone());
                            } else if let Some(existing_pos) = data.iter().position(|existing| {
                                (existing.timestamp - new_candle.timestamp).abs() < 1.0
                            }) {
                                data[existing_pos] = new_candle.clone();
                            }
                        }

                        while data.len() > 10000 {
                            data.pop_front();
                        }
                    }
                }

                if tx.send(candles).is_err() {
                    break;
                }
            }
            Err(e) => {
                eprintln!("Error fetching data: {}", e);
            }
        }

        let update_interval = match timeframe {
            Timeframe::M1 | Timeframe::M3 | Timeframe::M5 => 3,
            Timeframe::M15 | Timeframe::M30 => 30,
            Timeframe::H1 | Timeframe::H4 => 60,
            _ => 300,
        };

        tokio::time::sleep(tokio::time::Duration::from_secs(update_interval)).await;
    }
}

async fn fetch_klines_latest(
    timeframe: &Timeframe,
) -> Result<Vec<CandleData>, Box<dyn std::error::Error>> {
    let url = format!(
        "https://fapi.binance.com/fapi/v1/klines?symbol=BTCUSDT&interval={}&limit=500",
        timeframe.to_api_string()
    );

    let client = reqwest::Client::new();
    let response = client.get(&url).send().await?;

    if !response.status().is_success() {
        return Err(format!("API error: {}", response.status()).into());
    }

    let text = response.text().await?;
    let json: serde_json::Value = serde_json::from_str(&text)?;

    let mut candles = Vec::new();

    if let Some(array) = json.as_array() {
        for item in array {
            if let Some(kline_array) = item.as_array() {
                if kline_array.len() >= 11 {
                    let timestamp = kline_array[0].as_i64().unwrap_or(0) as f64;
                    let open = kline_array[1]
                        .as_str()
                        .unwrap_or("0")
                        .parse::<f64>()
                        .unwrap_or(0.0);
                    let high = kline_array[2]
                        .as_str()
                        .unwrap_or("0")
                        .parse::<f64>()
                        .unwrap_or(0.0);
                    let low = kline_array[3]
                        .as_str()
                        .unwrap_or("0")
                        .parse::<f64>()
                        .unwrap_or(0.0);
                    let close = kline_array[4]
                        .as_str()
                        .unwrap_or("0")
                        .parse::<f64>()
                        .unwrap_or(0.0);
                    let volume = kline_array[5]
                        .as_str()
                        .unwrap_or("0")
                        .parse::<f64>()
                        .unwrap_or(0.0);

                    if open > 0.0 && high > 0.0 && low > 0.0 && close > 0.0 {
                        candles.push(CandleData {
                            timestamp: timestamp / 1000.0,
                            open,
                            high,
                            low,
                            close,
                            volume,
                        });
                    }
                }
            }
        }
    }

    Ok(candles)
}

// Calculate 20-period Moving Average
fn calculate_ma20(data: &[CandleData]) -> Vec<(f64, f64)> {
    let mut ma_points = Vec::new();

    for i in 19..data.len() {
        let sum: f64 = data[i - 19..=i].iter().map(|candle| candle.close).sum();
        let ma_value = sum / 20.0;
        ma_points.push((data[i].timestamp, ma_value));
    }

    ma_points
}

// Calculate Bollinger Bands (20-period, 2 standard deviations)
fn calculate_bollinger_bands(
    data: &[CandleData],
) -> (Vec<(f64, f64)>, Vec<(f64, f64)>, Vec<(f64, f64)>) {
    let mut upper_band = Vec::new();
    let mut middle_band = Vec::new();
    let mut lower_band = Vec::new();

    for i in 19..data.len() {
        let window = &data[i - 19..=i];
        let sum: f64 = window.iter().map(|candle| candle.close).sum();
        let ma = sum / 20.0;

        // Calculate standard deviation
        let variance: f64 = window
            .iter()
            .map(|candle| {
                let diff = candle.close - ma;
                diff * diff
            })
            .sum::<f64>()
            / 20.0;

        let std_dev = variance.sqrt();
        let upper = ma + (2.0 * std_dev);
        let lower = ma - (2.0 * std_dev);

        let timestamp = data[i].timestamp;
        upper_band.push((timestamp, upper));
        middle_band.push((timestamp, ma));
        lower_band.push((timestamp, lower));
    }

    (upper_band, middle_band, lower_band)
}

// Calculate MACD (12, 26, 9)
fn calculate_macd(data: &[CandleData]) -> (Vec<(f64, f64)>, Vec<(f64, f64)>, Vec<(f64, f64)>) {
    if data.len() < 26 {
        return (Vec::new(), Vec::new(), Vec::new());
    }

    let mut ema12 = Vec::new();
    let mut ema26 = Vec::new();
    let mut macd_line = Vec::new();
    let mut signal_line = Vec::new();
    let mut histogram = Vec::new();

    // Calculate EMA12 and EMA26
    let alpha12 = 2.0 / (12.0 + 1.0);
    let alpha26 = 2.0 / (26.0 + 1.0);

    let mut ema12_value = data[0].close;
    let mut ema26_value = data[0].close;

    for (i, candle) in data.iter().enumerate() {
        if i == 0 {
            ema12.push(candle.close);
            ema26.push(candle.close);
        } else {
            ema12_value = alpha12 * candle.close + (1.0 - alpha12) * ema12_value;
            ema26_value = alpha26 * candle.close + (1.0 - alpha26) * ema26_value;
            ema12.push(ema12_value);
            ema26.push(ema26_value);
        }

        if i >= 25 {
            let macd_value = ema12[i] - ema26[i];
            macd_line.push((candle.timestamp, macd_value));
        }
    }

    // Calculate Signal line (9-period EMA of MACD)
    if !macd_line.is_empty() {
        let alpha9 = 2.0 / (9.0 + 1.0);
        let mut signal_value = macd_line[0].1;

        for (i, (timestamp, macd_val)) in macd_line.iter().enumerate() {
            if i == 0 {
                signal_line.push((*timestamp, *macd_val));
                signal_value = *macd_val;
            } else {
                signal_value = alpha9 * macd_val + (1.0 - alpha9) * signal_value;
                signal_line.push((*timestamp, signal_value));
            }

            if i >= 8 {
                let hist_value = macd_val - signal_line[i].1;
                histogram.push((*timestamp, hist_value));
            }
        }
    }

    (macd_line, signal_line, histogram)
}

// Calculate RSI (14-period)
fn calculate_rsi(data: &[CandleData]) -> Vec<(f64, f64)> {
    if data.len() < 15 {
        return Vec::new();
    }

    let mut rsi_points = Vec::new();
    let period = 14;

    for i in period..data.len() {
        let mut gains = 0.0;
        let mut losses = 0.0;

        for j in (i - period + 1)..=i {
            let change = data[j].close - data[j - 1].close;
            if change > 0.0 {
                gains += change;
            } else {
                losses += change.abs();
            }
        }

        let avg_gain = gains / period as f64;
        let avg_loss = losses / period as f64;

        let rs = if avg_loss != 0.0 {
            avg_gain / avg_loss
        } else {
            100.0
        };
        let rsi = 100.0 - (100.0 / (1.0 + rs));

        rsi_points.push((data[i].timestamp, rsi));
    }

    rsi_points
}

impl eframe::App for CryptoApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.is_dragging {
            if let Some(receiver) = &mut self.data_receiver {
                while let Ok(new_candles) = receiver.try_recv() {
                    if !new_candles.is_empty() {
                        self.is_loading = false;

                        if let Some(latest) = new_candles.last() {
                            self.latest_timestamp = latest.timestamp;
                            self.trading_panel.current_price = latest.close;
                            
                            // earliest_timestamp 초기화 (첫 로드시)
                            if self.earliest_timestamp == 0.0 {
                                if let Ok(data) = self.candle_data.lock() {
                                    if let Some(first) = data.front() {
                                        self.earliest_timestamp = first.timestamp;
                                    }
                                }
                            }

                            // 기존 뷰 윈도우 로직...
                            if self.view_window_start == 0.0 {
                                let right_margin = self.window_size * 0.1;
                                self.view_window_start =
                                    self.latest_timestamp - self.window_size + right_margin;
                                self.is_live_mode = true;
                            } else if self.is_live_mode {
                                let right_margin = self.window_size * 0.1;
                                let buffer = match self.timeframe {
                                    Timeframe::M1 => 60.0 * 5.0,
                                    Timeframe::M3 => 60.0 * 15.0,
                                    Timeframe::M5 => 60.0 * 25.0,
                                    Timeframe::M15 => 60.0 * 75.0,
                                    Timeframe::M30 => 60.0 * 150.0,
                                    Timeframe::H1 => 60.0 * 60.0 * 5.0,
                                    Timeframe::H4 => 60.0 * 60.0 * 20.0,
                                    _ => 60.0 * 60.0 * 24.0 * 5.0,
                                };
                                self.view_window_start = self.latest_timestamp + buffer
                                    - self.window_size
                                    + right_margin;
                            }
                        }
                    }
                }
            }
            
            // 과거 데이터 수신 처리
            if let Some(receiver) = &mut self.historical_data_receiver {
                while let Ok(historical_candles) = receiver.try_recv() {
                    if !historical_candles.is_empty() {
                        // earliest_timestamp 업데이트
                        if let Some(earliest) = historical_candles.first() {
                            if earliest.timestamp < self.earliest_timestamp {
                                self.earliest_timestamp = earliest.timestamp;
                            }
                        }
                    }
                    self.is_loading_historical = false;
                }
            }
        }
        
        // 과거 데이터 필요 여부 확인
        self.check_and_load_historical_data();
        // Check for new data
        if !self.is_dragging {
            if let Some(receiver) = &mut self.data_receiver {
                while let Ok(new_candles) = receiver.try_recv() {
                    if !new_candles.is_empty() {
                        self.is_loading = false;

                        if let Some(latest) = new_candles.last() {
                            self.latest_timestamp = latest.timestamp;
                            self.trading_panel.current_price = latest.close;

                            if self.view_window_start == 0.0 {
                                let right_margin = self.window_size * 0.1;
                                self.view_window_start =
                                    self.latest_timestamp - self.window_size + right_margin;
                                self.is_live_mode = true;
                            } else if self.is_live_mode {
                                let right_margin = self.window_size * 0.1;
                                let buffer = match self.timeframe {
                                    Timeframe::M1 => 60.0 * 5.0,
                                    Timeframe::M3 => 60.0 * 15.0,
                                    Timeframe::M5 => 60.0 * 25.0,
                                    Timeframe::M15 => 60.0 * 75.0,
                                    Timeframe::M30 => 60.0 * 150.0,
                                    Timeframe::H1 => 60.0 * 60.0 * 5.0,
                                    Timeframe::H4 => 60.0 * 60.0 * 20.0,
                                    _ => 60.0 * 60.0 * 24.0 * 5.0,
                                };
                                self.view_window_start = self.latest_timestamp + buffer
                                    - self.window_size
                                    + right_margin;
                            }
                        }
                    }
                }
            }
        }

        // Top controls
        egui::TopBottomPanel::top("control_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Timeframe:");
                let old_timeframe = self.timeframe.clone();
                egui::ComboBox::from_id_salt("timeframe")
                    .selected_text(self.timeframe.to_display_string())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.timeframe, Timeframe::M1, "1m");
                        ui.selectable_value(&mut self.timeframe, Timeframe::M3, "3m");
                        ui.selectable_value(&mut self.timeframe, Timeframe::M5, "5m");
                        ui.selectable_value(&mut self.timeframe, Timeframe::M15, "15m");
                        ui.selectable_value(&mut self.timeframe, Timeframe::M30, "30m");
                        ui.selectable_value(&mut self.timeframe, Timeframe::H1, "1h");
                        ui.selectable_value(&mut self.timeframe, Timeframe::H4, "4h");
                        ui.selectable_value(&mut self.timeframe, Timeframe::H12, "12h");
                        ui.selectable_value(&mut self.timeframe, Timeframe::D1, "1d");
                        ui.selectable_value(&mut self.timeframe, Timeframe::W1, "1w");
                        ui.selectable_value(&mut self.timeframe, Timeframe::MN1, "1M");
                    });

                if old_timeframe != self.timeframe {
                    self.window_size = self.timeframe.get_window_size();
                    self.is_loading = true;

                    if let Ok(mut data) = self.candle_data.lock() {
                        data.clear();
                    }

                    if let Some(rt) = &self.runtime {
                        let (tx, rx) = mpsc::unbounded_channel();
                        self.data_receiver = Some(rx);

                        let candle_data_clone = self.candle_data.clone();
                        let timeframe_clone = self.timeframe.clone();
                        rt.spawn(fetch_binance_data(tx, candle_data_clone, timeframe_clone));
                    }

                    self.view_window_start = 0.0;
                    self.is_live_mode = true;
                }

                ui.separator();

                ui.label("Chart:");
                egui::ComboBox::from_id_salt("chart_type")
                    .selected_text(match self.chart_type {
                        ChartType::Line => "Line",
                        ChartType::Candlestick => "Candle",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.chart_type, ChartType::Line, "Line");
                        ui.selectable_value(&mut self.chart_type, ChartType::Candlestick, "Candle");
                    });

                ui.separator();

                ui.label("Indicators:");
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.show_ma20, "MA20");
                    ui.checkbox(&mut self.show_bollinger, "Bollinger");
                    ui.checkbox(&mut self.show_macd, "MACD");
                    ui.checkbox(&mut self.show_rsi, "RSI");
                    ui.checkbox(&mut self.show_volume, "Volume");
                });

                ui.separator();

                if ui.button("Live").clicked() {
                    let right_margin = self.window_size * 0.1;
                    let buffer = match self.timeframe {
                        Timeframe::M1 => 60.0 * 5.0,
                        Timeframe::M3 => 60.0 * 15.0,
                        Timeframe::M5 => 60.0 * 25.0,
                        Timeframe::M15 => 60.0 * 75.0,
                        Timeframe::M30 => 60.0 * 150.0,
                        Timeframe::H1 => 60.0 * 60.0 * 5.0,
                        Timeframe::H4 => 60.0 * 60.0 * 20.0,
                        _ => 60.0 * 60.0 * 24.0 * 5.0,
                    };
                    self.view_window_start =
                        self.latest_timestamp + buffer - self.window_size + right_margin;
                    self.is_live_mode = true;
                }

                ui.separator();

                if !self.trading_panel.current_price.is_nan()
                    && self.trading_panel.current_price > 0.0
                {
                    ui.colored_label(
                        egui::Color32::WHITE,
                        format!("Price: ${:.2}", self.trading_panel.current_price),
                    );
                }

                if self.is_loading {
                    ui.colored_label(egui::Color32::YELLOW, "Loading...");
                } else {
                    if self.is_live_mode {
                        ui.colored_label(egui::Color32::GREEN, "🔴 LIVE");
                    } else {
                        ui.colored_label(egui::Color32::LIGHT_BLUE, "📜 History");
                    }
                }
            });
        });

        // Main layout with side panel for trading
        egui::SidePanel::right("trading_panel")
            .min_width(300.0)
            .show(ctx, |ui| {
                ui.heading("💰 Trading");
                ui.separator();

                // Balance display
                ui.group(|ui| {
                    ui.label("💳 Balance");
                    ui.label(format!("USDT: ${:.2}", self.trading_panel.balance_usdt));
                    ui.label(format!("BTC: {:.6}", self.trading_panel.balance_btc));
                });

                ui.separator();

                // Order type
                ui.horizontal(|ui| {
                    ui.label("Order:");
                    ui.selectable_value(
                        &mut self.trading_panel.order_type,
                        OrderType::Buy,
                        "🟢 Buy",
                    );
                    ui.selectable_value(
                        &mut self.trading_panel.order_type,
                        OrderType::Sell,
                        "🔴 Sell",
                    );
                });

                // Order mode
                ui.horizontal(|ui| {
                    ui.label("Type:");
                    ui.selectable_value(
                        &mut self.trading_panel.order_mode,
                        OrderMode::Market,
                        "Market",
                    );
                    ui.selectable_value(
                        &mut self.trading_panel.order_mode,
                        OrderMode::Limit,
                        "Limit",
                    );
                });

                ui.separator();

                // Quantity input
                ui.horizontal(|ui| {
                    ui.label("Quantity:");
                    ui.text_edit_singleline(&mut self.trading_panel.quantity);
                    ui.label("BTC");
                });

                // Price input (only for limit orders)
                if self.trading_panel.order_mode == OrderMode::Limit {
                    ui.horizontal(|ui| {
                        ui.label("Price:");
                        ui.text_edit_singleline(&mut self.trading_panel.price);
                        ui.label("USDT");
                    });
                } else {
                    // Show current price for market orders
                    if !self.trading_panel.current_price.is_nan()
                        && self.trading_panel.current_price > 0.0
                    {
                        ui.horizontal(|ui| {
                            ui.label("Est. Price:");
                            ui.colored_label(
                                egui::Color32::YELLOW,
                                format!("${:.2}", self.trading_panel.current_price),
                            );
                        });
                    }
                }

                ui.separator();

                // Order button
                let button_color = match self.trading_panel.order_type {
                    OrderType::Buy => egui::Color32::from_rgb(0, 200, 100),
                    OrderType::Sell => egui::Color32::from_rgb(255, 100, 100),
                };

                let button_text = match (
                    &self.trading_panel.order_type,
                    &self.trading_panel.order_mode,
                ) {
                    (OrderType::Buy, OrderMode::Market) => "Market Buy",
                    (OrderType::Buy, OrderMode::Limit) => "Limit Buy",
                    (OrderType::Sell, OrderMode::Market) => "Market Sell",
                    (OrderType::Sell, OrderMode::Limit) => "Limit Sell",
                };

                if ui
                    .add_sized(
                        [ui.available_width(), 40.0],
                        egui::Button::new(button_text).fill(button_color),
                    )
                    .clicked()
                {
                    // Order processing (virtual trading)
                    if let Ok(quantity) = self.trading_panel.quantity.parse::<f64>() {
                        let price = if self.trading_panel.order_mode == OrderMode::Market {
                            self.trading_panel.current_price
                        } else {
                            self.trading_panel.price.parse::<f64>().unwrap_or(0.0)
                        };

                        if price > 0.0 && quantity > 0.0 {
                            match self.trading_panel.order_type {
                                OrderType::Buy => {
                                    let total_cost = price * quantity;
                                    if total_cost <= self.trading_panel.balance_usdt {
                                        self.trading_panel.balance_usdt -= total_cost;
                                        self.trading_panel.balance_btc += quantity;
                                    }
                                }
                                OrderType::Sell => {
                                    if quantity <= self.trading_panel.balance_btc {
                                        self.trading_panel.balance_btc -= quantity;
                                        self.trading_panel.balance_usdt += price * quantity;
                                    }
                                }
                            }
                        }
                    }
                }

                ui.separator();

                // Quick order buttons
                ui.label("⚡ Quick Order:");
                ui.horizontal(|ui| {
                    if ui.small_button("25%").clicked() {
                        match self.trading_panel.order_type {
                            OrderType::Buy => {
                                if self.trading_panel.current_price > 0.0 {
                                    let amount = (self.trading_panel.balance_usdt * 0.25)
                                        / self.trading_panel.current_price;
                                    self.trading_panel.quantity = format!("{:.6}", amount);
                                }
                            }
                            OrderType::Sell => {
                                let amount = self.trading_panel.balance_btc * 0.25;
                                self.trading_panel.quantity = format!("{:.6}", amount);
                            }
                        }
                    }
                    if ui.small_button("50%").clicked() {
                        match self.trading_panel.order_type {
                            OrderType::Buy => {
                                if self.trading_panel.current_price > 0.0 {
                                    let amount = (self.trading_panel.balance_usdt * 0.5)
                                        / self.trading_panel.current_price;
                                    self.trading_panel.quantity = format!("{:.6}", amount);
                                }
                            }
                            OrderType::Sell => {
                                let amount = self.trading_panel.balance_btc * 0.5;
                                self.trading_panel.quantity = format!("{:.6}", amount);
                            }
                        }
                    }
                    if ui.small_button("100%").clicked() {
                        match self.trading_panel.order_type {
                            OrderType::Buy => {
                                if self.trading_panel.current_price > 0.0 {
                                    let amount = self.trading_panel.balance_usdt
                                        / self.trading_panel.current_price;
                                    self.trading_panel.quantity = format!("{:.6}", amount);
                                }
                            }
                            OrderType::Sell => {
                                self.trading_panel.quantity =
                                    format!("{:.6}", self.trading_panel.balance_btc as f64);
                            }
                        }
                    }
                });
            });

        // Chart area (now takes remaining space)
        // Chart area (now takes remaining space)
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(format!(
                "📊 BTC/USDT ({})",
                self.timeframe.to_display_string()
            ));

            if self.is_loading {
                ui.centered_and_justified(|ui| {
                    ui.colored_label(egui::Color32::YELLOW, "Loading data...");
                });
                return;
            }

            // 데이터 복제를 먼저 수행하여 borrowing 문제 해결
            let chart_data = {
                let data = self.candle_data.lock().unwrap();
                if data.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.colored_label(egui::Color32::RED, "No data available");
                    });
                    return;
                }
                data.iter().cloned().collect::<Vec<_>>()
            };

            let mut view_window_start = self.view_window_start;
            let window_size = self.window_size;
            let latest_timestamp = self.latest_timestamp;
            let chart_type = self.chart_type.clone();
            let candle_width = self.candle_width;
            let candle_interval = self.timeframe.get_candle_interval();

            // 오른쪽 여백 설정 - 전체 윈도우의 10%
            let right_margin = window_size * 0.1;
            let effective_window_end = view_window_start + window_size - right_margin;

            // 드래그 상태 초기화
            self.shared_drag_delta = 0.0;
            self.any_chart_dragged = false;

            // Calculate available height for charts
            let available_height = ui.available_height();
            let mut total_charts = 1; // Main price chart

            if self.show_macd {
                total_charts += 1;
            }
            if self.show_rsi {
                total_charts += 1;
            }

            // Distribute heights: main chart gets 70%, indicators share 30%
            let main_chart_height = available_height * 0.7;
            let indicator_height = if total_charts > 1 {
                (available_height * 0.3) / (total_charts - 1) as f32
            } else {
                0.0
            };

            // Main Price Chart
            ui.allocate_ui_with_layout(
                egui::Vec2::new(ui.available_width(), main_chart_height),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    let filtered_data: Vec<_> = chart_data
                        .iter()
                        .filter(|candle| {
                            let margin = window_size * 0.1;
                            candle.timestamp >= (view_window_start - margin)
                                && candle.timestamp <= (effective_window_end + margin)
                        })
                        .cloned()
                        .collect();

                    let (price_min, price_max) = self.calculate_visible_price_range(&filtered_data);

                    let plot = Plot::new("price_chart")
                    .view_aspect(3.0)
                    .allow_zoom([false, false])
                    .allow_drag([true, false])
                    .allow_scroll(false)
                    .auto_bounds(egui::Vec2b::new(false, false))
                    .show_axes([false, true])  // X축도 표시하도록 변경
                    .y_axis_width(5)
                    .default_x_bounds(view_window_start, effective_window_end)
                    .default_y_bounds(price_min, price_max)
                    .y_axis_position(egui_plot::HPlacement::Right);

                    plot.show(ui, |plot_ui| {
                        // 드래그 처리
                        if plot_ui.response().dragged() {
                            let drag_delta = plot_ui.pointer_coordinate_drag_delta();
                            if drag_delta.x.abs() > 0.1 {
                                self.shared_drag_delta = drag_delta.x as f64;
                                self.any_chart_dragged = true;
                            }
                        }

                        match chart_type {
                            ChartType::Line => {
                                let price_points: PlotPoints = filtered_data
                                    .iter()
                                    .map(|candle| [candle.timestamp, candle.close])
                                    .collect();

                                let price_line = Line::new("Close Price", price_points)
                                    .color(egui::Color32::from_rgb(100, 200, 255))
                                    .width(2.0);

                                plot_ui.line(price_line);
                            }
                            ChartType::Candlestick => {
                                let mut box_elements = Vec::new();

                                for candle in &filtered_data {
                                    let is_bullish = candle.close >= candle.open;
                                    let color = if is_bullish {
                                        egui::Color32::from_rgb(0, 255, 150)
                                    } else {
                                        egui::Color32::from_rgb(255, 80, 80)
                                    };

                                    let box_spread = BoxSpread::new(
                                        candle.low,
                                        candle.open.min(candle.close),
                                        (candle.open + candle.close) / 2.0,
                                        candle.open.max(candle.close),
                                        candle.high,
                                    );

                                    let actual_candle_width = candle_interval * candle_width * 0.8;

                                    let box_elem = BoxElem::new(candle.timestamp, box_spread)
                                        .whisker_width(actual_candle_width * 0.1)
                                        .box_width(actual_candle_width)
                                        .fill(color)
                                        .stroke(egui::Stroke::new(1.5, color));

                                    box_elements.push(box_elem);
                                }

                                let candlestick_plot = BoxPlot::new("Candlestick", box_elements);
                                plot_ui.box_plot(candlestick_plot);
                            }
                        }

                        // Add Bollinger Bands
                        if self.show_bollinger && filtered_data.len() >= 20 {
                            let (upper_band, middle_band, lower_band) =
                                calculate_bollinger_bands(&filtered_data);

                            if !upper_band.is_empty() {
                                let upper_points: PlotPoints =
                                    upper_band.iter().map(|(t, v)| [*t, *v]).collect();
                                let upper_line = Line::new("BB Upper", upper_points)
                                    .color(egui::Color32::from_rgb(128, 128, 128))
                                    .width(1.5);
                                plot_ui.line(upper_line);

                                let lower_points: PlotPoints =
                                    lower_band.iter().map(|(t, v)| [*t, *v]).collect();
                                let lower_line = Line::new("BB Lower", lower_points)
                                    .color(egui::Color32::from_rgb(128, 128, 128))
                                    .width(1.5);
                                plot_ui.line(lower_line);

                                if !self.show_ma20 {
                                    let middle_points: PlotPoints =
                                        middle_band.iter().map(|(t, v)| [*t, *v]).collect();
                                    let middle_line = Line::new("BB Middle", middle_points)
                                        .color(egui::Color32::from_rgb(255, 215, 0))
                                        .width(1.5);
                                    plot_ui.line(middle_line);
                                }
                            }
                        }

                        // Add MA20 line
                        if self.show_ma20 && filtered_data.len() >= 20 {
                            let ma20_points = calculate_ma20(&filtered_data);
                            if !ma20_points.is_empty() {
                                let ma20_plot_points: PlotPoints = ma20_points
                                    .iter()
                                    .map(|(timestamp, ma_value)| [*timestamp, *ma_value])
                                    .collect();

                                let ma20_line = Line::new("MA20", ma20_plot_points)
                                    .color(egui::Color32::from_rgb(255, 215, 0))
                                    .width(2.5);

                                plot_ui.line(ma20_line);
                            }
                        }
                    });
                },
            );

            // Volume overlay on price chart (if enabled)
            if self.show_volume {
                ui.allocate_ui_with_layout(
                    egui::Vec2::new(ui.available_width(), main_chart_height * 0.25),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.style_mut().visuals.widgets.inactive.bg_fill =
                            egui::Color32::TRANSPARENT;

                        let volume_plot = Plot::new("volume_overlay")
                            .allow_zoom([false, false])
                            .allow_drag([true, false])
                            .allow_scroll(false)
                            .auto_bounds(egui::Vec2b::new(false, true))
                            .default_x_bounds(view_window_start, effective_window_end)
                            .y_axis_width(5)  // 이 줄 추가

                            .show_background(false)
                            .show_axes([false, true])
                            .y_axis_position(egui_plot::HPlacement::Right);

                        volume_plot.show(ui, |plot_ui| {
                            // 드래그 처리
                            if plot_ui.response().dragged() {
                                let drag_delta = plot_ui.pointer_coordinate_drag_delta();
                                if drag_delta.x.abs() > 0.1 {
                                    self.shared_drag_delta = drag_delta.x as f64;
                                    self.any_chart_dragged = true;
                                }
                            }

                            let filtered_data: Vec<_> = chart_data
                                .iter()
                                .filter(|candle| {
                                    let margin = window_size * 0.1;
                                    candle.timestamp >= (view_window_start - margin)
                                        && candle.timestamp <= (effective_window_end + margin)
                                })
                                .cloned()
                                .collect();

                            let mut volume_bars = Vec::new();
                            for candle in &filtered_data {
                                let color = if candle.close >= candle.open {
                                    egui::Color32::from_rgba_unmultiplied(0, 200, 100, 100)
                                } else {
                                    egui::Color32::from_rgba_unmultiplied(255, 100, 100, 100)
                                };

                                let actual_candle_width = candle_interval * candle_width * 0.8;
                                let box_spread = BoxSpread::new(
                                    0.0,
                                    0.0,
                                    candle.volume / 2.0,
                                    candle.volume,
                                    candle.volume,
                                );
                                let box_elem = BoxElem::new(candle.timestamp, box_spread)
                                    .box_width(actual_candle_width)
                                    .fill(color)
                                    .stroke(egui::Stroke::new(0.5, color));

                                volume_bars.push(box_elem);
                            }

                            let volume_plot = BoxPlot::new("Volume", volume_bars);
                            plot_ui.box_plot(volume_plot);
                        });
                    },
                );
            }

            // MACD Chart
            if self.show_macd {
                ui.allocate_ui_with_layout(
                    egui::Vec2::new(ui.available_width(), indicator_height),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        ui.label("📈 MACD");
                        let macd_plot = Plot::new("macd_chart")
                            .allow_zoom([false, false])
                            .allow_drag([true, false])
                            .allow_scroll(false)
                            .auto_bounds(egui::Vec2b::new(false, true))
                            .default_x_bounds(view_window_start, effective_window_end)
                            .show_axes([false, true])
                            .y_axis_width(5)  // 이 줄 추가

                            .y_axis_position(egui_plot::HPlacement::Right);

                        macd_plot.show(ui, |plot_ui| {
                            // 드래그 처리
                            if plot_ui.response().dragged() {
                                let drag_delta = plot_ui.pointer_coordinate_drag_delta();
                                if drag_delta.x.abs() > 0.1 {
                                    self.shared_drag_delta = drag_delta.x as f64;
                                    self.any_chart_dragged = true;
                                }
                            }

                            let filtered_data: Vec<_> = chart_data
                                .iter()
                                .filter(|candle| {
                                    let margin = window_size * 0.1;
                                    candle.timestamp >= (view_window_start - margin)
                                        && candle.timestamp <= (effective_window_end + margin)
                                })
                                .cloned()
                                .collect();

                            if filtered_data.len() >= 35 {
                                let (macd_line, signal_line, histogram) =
                                    calculate_macd(&filtered_data);

                                // MACD Line
                                if !macd_line.is_empty() {
                                    let macd_points: PlotPoints =
                                        macd_line.iter().map(|(t, v)| [*t, *v]).collect();
                                    let macd = Line::new("MACD", macd_points)
                                        .color(egui::Color32::from_rgb(0, 150, 255))
                                        .width(2.0);
                                    plot_ui.line(macd);
                                }

                                // Signal Line
                                if !signal_line.is_empty() {
                                    let signal_points: PlotPoints =
                                        signal_line.iter().map(|(t, v)| [*t, *v]).collect();
                                    let signal = Line::new("Signal", signal_points)
                                        .color(egui::Color32::from_rgb(255, 150, 0))
                                        .width(2.0);
                                    plot_ui.line(signal);
                                }

                                // Histogram
                                if !histogram.is_empty() {
                                    let mut hist_bars = Vec::new();
                                    for (timestamp, value) in &histogram {
                                        let color = if *value >= 0.0 {
                                            egui::Color32::from_rgb(0, 200, 100)
                                        } else {
                                            egui::Color32::from_rgb(255, 100, 100)
                                        };

                                        let actual_width = candle_interval * 0.5;
                                        let box_spread = if *value >= 0.0 {
                                            BoxSpread::new(0.0, 0.0, value / 2.0, *value, *value)
                                        } else {
                                            BoxSpread::new(*value, *value, value / 2.0, 0.0, 0.0)
                                        };

                                        let box_elem = BoxElem::new(*timestamp, box_spread)
                                            .box_width(actual_width)
                                            .fill(color)
                                            .stroke(egui::Stroke::new(1.0, color));

                                        hist_bars.push(box_elem);
                                    }

                                    let hist_plot = BoxPlot::new("MACD Histogram", hist_bars);
                                    plot_ui.box_plot(hist_plot);
                                }
                            }
                        });
                    },
                );
            }

            // RSI Chart
            if self.show_rsi {
                ui.allocate_ui_with_layout(
                    egui::Vec2::new(ui.available_width(), indicator_height),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        ui.label("⚡ RSI");
                        let rsi_plot = Plot::new("rsi_chart")
                            .allow_zoom([false, false])
                            .allow_drag([true, false])
                            .allow_scroll(false)
                            .auto_bounds(egui::Vec2b::new(false, false))
                            .default_x_bounds(view_window_start, effective_window_end)
                            .default_y_bounds(0.0, 100.0)
                            .show_axes([true, true])
                            .x_axis_formatter(|mark, _range| {
                                format_timestamp_to_date(mark.value, &self.timeframe)
                            })
                            .y_axis_width(5)  // 이 줄 추가

                            .y_axis_position(egui_plot::HPlacement::Right);

                        rsi_plot.show(ui, |plot_ui| {
                            // 드래그 처리
                            if plot_ui.response().dragged() {
                                let drag_delta = plot_ui.pointer_coordinate_drag_delta();
                                if drag_delta.x.abs() > 0.1 {
                                    self.shared_drag_delta = drag_delta.x as f64;
                                    self.any_chart_dragged = true;
                                }
                            }

                            let filtered_data: Vec<_> = chart_data
                                .iter()
                                .filter(|candle| {
                                    let margin = window_size * 0.1;
                                    candle.timestamp >= (view_window_start - margin)
                                        && candle.timestamp <= (effective_window_end + margin)
                                })
                                .cloned()
                                .collect();

                            if filtered_data.len() >= 15 {
                                let rsi_points = calculate_rsi(&filtered_data);

                                if !rsi_points.is_empty() {
                                    let rsi_plot_points: PlotPoints =
                                        rsi_points.iter().map(|(t, v)| [*t, *v]).collect();
                                    let rsi_line = Line::new("RSI", rsi_plot_points)
                                        .color(egui::Color32::from_rgb(255, 100, 255))
                                        .width(2.0);
                                    plot_ui.line(rsi_line);

                                    // Add RSI reference lines
                                    let overbought: PlotPoints = vec![
                                        [view_window_start, 70.0],
                                        [effective_window_end, 70.0],
                                    ]
                                    .into();
                                    let oversold: PlotPoints = vec![
                                        [view_window_start, 30.0],
                                        [effective_window_end, 30.0],
                                    ]
                                    .into();
                                    let middle: PlotPoints = vec![
                                        [view_window_start, 50.0],
                                        [effective_window_end, 50.0],
                                    ]
                                    .into();

                                    let overbought_line = Line::new("Overbought", overbought)
                                        .color(egui::Color32::from_rgb(255, 100, 100))
                                        .width(1.0);
                                    let oversold_line = Line::new("Oversold", oversold)
                                        .color(egui::Color32::from_rgb(100, 255, 100))
                                        .width(1.0);
                                    let middle_line = Line::new("Middle", middle)
                                        .color(egui::Color32::from_rgb(128, 128, 128))
                                        .width(1.0);

                                    plot_ui.line(overbought_line);
                                    plot_ui.line(oversold_line);
                                    plot_ui.line(middle_line);
                                }
                            }
                        });
                    },
                );
            }

            // 모든 차트 처리 후 공유 드래그 적용
            // 모든 차트 처리 후 공유 드래그 적용
            if self.any_chart_dragged && self.shared_drag_delta.abs() > 0.1 {
                let proposed_start = view_window_start - self.shared_drag_delta as f64;
                let proposed_end = proposed_start + window_size - right_margin; // right_margin_seconds -> right_margin

                if proposed_end <= latest_timestamp && proposed_start >= 0.0 {
                    self.view_window_start = proposed_start;
                    self.is_live_mode = false;
                } else if proposed_end > latest_timestamp {
                    self.view_window_start = latest_timestamp - window_size + right_margin; // right_margin_seconds -> right_margin
                    self.is_live_mode = true;
                } else if proposed_start < 0.0 {
                    self.view_window_start = 0.0;
                    self.is_live_mode = false;
                }

                self.is_dragging = true;
            } else {
                self.is_dragging = false;
            }
        });

        // Repaint every second for live updates
        ctx.request_repaint_after(std::time::Duration::from_secs(1));
    }
}
