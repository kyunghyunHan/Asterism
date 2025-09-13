use dotenv::dotenv;
mod api;
mod models;
mod trading;
mod ui;
mod utils;
use crate::models::SignalScoring;
use api::{
    account::binance_account_connection,
    binance::{binance_connection, fetch_candles, fetch_candles_async, get_top_volume_pairs},
    excution::execute_trade,
    BinanceTrade, FuturesAccountInfo,
};
use iced::{
    futures::channel::mpsc,
    time::{Duration, Instant},
    widget::{canvas::Canvas, container, pane_grid, pick_list, text, Column, Container, Row, Text},
    Element, Length,
    Length::FillPortion,
    Size, Subscription,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, VecDeque};
use trading::{
    markey_order::{market_buy, market_sell},
    TradeType,
};
use ui::chart::calculate_scored_signals;
use ui::{
    buttons::ma_controls,
    infos::{account_info, coin_info, current_position},
    trading::{auto_trading_toggle, order_buttons},
    CandleType, Candlestick, Chart, ChartState,
};
use utils::{constant as uc, logs as ul};
//Main
pub struct Futurx {
    panes: pane_grid::State<Pane>,
    candlesticks: BTreeMap<u64, Candlestick>, // 캔들스틱 데이터 저장
    selected_coin: String,                    // 현재 선택된 코인
    pub selected_candle_type: CandleType,     // 선택된 캔들 타입 (1분,3분,일봉)
    coin_list: HashMap<String, CoinInfo>,     // 코인 목록 정보
    auto_scroll: bool,                        // 자동 스크롤 여부
    ws_sender: Option<mpsc::Sender<String>>,  // WebSocket 메시지 전송자
    show_ma5: bool,                           // 5일 이동평균선 표시 여부
    show_ma10: bool,                          // 10일 이동평균선 표시 여부
    show_ma20: bool,                          // 20일 이동평균선 표시 여부
    show_ma200: bool,                         // 200일 이동평균선 표시 여부
    loading_more: bool,                       // 추가 데이터 로딩 중 여부
    oldest_date: Option<String>,              // 가장 오래된 캔들 날짜
    account_info: Option<FuturesAccountInfo>, // 계좌 정보
    alerts: VecDeque<Alert>,                  // 알림 메시지 큐
    auto_trading_enabled: bool,               // 자동매매 활성화 상태
    last_trade_time: Option<Instant>,         // 마지막 거래 시간
    alert_sender: mpsc::Sender<(String, AlertType)>, // 알림 메시지 전송자
    average_prices: HashMap<String, f64>,     // 평균 가격 정보

    scored_signals_enabled: bool,
    buy_scored_signals: BTreeMap<u64, SignalScoring>,
    sell_scored_signals: BTreeMap<u64, SignalScoring>,
}
enum Pane {
    Chart,
    LeftSidebar,
    RightSidebar,
}
#[derive(Debug, Clone)]
struct Alert {
    message: String,       // 알림 메시지 내용
    alert_type: AlertType, // 알림 타입
    timestamp: Instant,    // 알림 발생 시간
}
#[derive(Debug, Clone)]
enum AlertType {
    Buy,   // 매수 신호
    Sell,  // 매도 신호
    Info,  // 일반 정보
    Error, // 에러
}

#[derive(Debug, Deserialize, Clone)]
struct Trade {
    symbol: String,
    id: u64,
    price: String,
    qty: String,
    #[serde(rename = "quoteQty")]
    quote_qty: String,
    #[serde(rename = "isBuyer")]
    is_buyer: bool,
    time: u64,
}
#[derive(Debug, Clone)]
pub enum Message {
    PaneDragged(pane_grid::DragEvent),             // 매개변수 필요
    PaneResized(pane_grid::ResizeEvent),           // 매개변수 필요
    AddCandlestick((u64, BinanceTrade)),           // 캔들스틱 추가
    RemoveCandlestick,                             // 캔들스틱 제거
    SelectCoin(String),                            // 코인 선택
    UpdateCoinPrice(String, f64, f64),             // 코인 가격 업데이트
    SelectCandleType(CandleType),                  // 캔들 타입 선택
    Error,                                         // 에러 발생
    WebSocketInit(mpsc::Sender<String>),           // WebSocket 초기화
    UpdatePrice(String, f64, f64),                 // 가격 업데이트
    ToggleMA5,                                     // 5일 이동평균선 토글
    ToggleMA10,                                    // 10일 이동평균선 토글
    ToggleMA20,                                    // 20일 이동평균선 토글
    ToggleMA200,                                   // 200일 이동평균선 토글
    LoadMoreCandles,                               // 추가 캔들 로드
    MoreCandlesLoaded(BTreeMap<u64, Candlestick>), // 추가 캔들 로드 완료
    TryBuy {
        // 매수 시도
        price: f64,
        strength: f32,
        timestamp: u64,
        indicators: TradeIndicators,
    },
    TrySell {
        // 매도 시도
        price: f64,
        strength: f32,
        timestamp: u64,
        indicators: TradeIndicators,
    },
    UpdateAccountInfo(FuturesAccountInfo), // 계좌 정보 업데이트
    FetchError(String),                    // 데이터 가져오기 에러
    AddAlert(String, AlertType),           // 알림 추가
    RemoveAlert,                           // 알림 제거
    Tick,                                  // 타이머 틱
    ToggleAutoTrading,                     // 자동매매 토글
    MarketBuy,                             // 시장가 매수
    MarketSell,                            // 시장가 매도
    UpdateAveragePrice(String, f64),       // 평균가격 업데이트
    ToggleScoredSignals,
}
//코인 정보 구조체
#[derive(Debug, Clone)]
struct CoinInfo {
    symbol: String, // 코인 심볼
    name: String,   // 코인 이름
    price: f64,     // 현재 가격
}

// 거래 지표 정보를 담는 구조체
#[derive(Debug, Clone)]
pub struct TradeIndicators {
    rsi: f32,          // RSI 지표
    ma5: f32,          // 5일 이동평균
    ma20: f32,         // 20일 이동평균
    volume_ratio: f32, // 거래량 비율
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OrderBookEntry {}
#[derive(Debug, Clone)]
pub struct OrderBool {}

impl Default for Futurx {
    fn default() -> Self {
        // 거래량 상위 20개 코인 가져오기
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let top_pairs = runtime.block_on(async {
            match get_top_volume_pairs().await {
                Ok(pairs) => pairs,
                Err(e) => {
                    println!("Error fetching top pairs: {}", e);
                    vec![] // 에러 시 빈 벡터 반환
                }
            }
        });

        let mut coin_list = HashMap::new();

        // 상위 20개 코인으로 초기화
        for (symbol, _volume) in top_pairs {
            let symbol = symbol.strip_suffix("USDT").unwrap_or(&symbol);
            coin_list.insert(
                symbol.to_string(),
                CoinInfo {
                    symbol: format!("{}-USDT", symbol),
                    name: symbol.to_string(),
                    price: 0.0,
                },
            );
        }

        // 만약 API 호출이 실패하면 기본 리스트 사용
        if coin_list.is_empty() {
            for symbol in &uc::DEFAULT_ARR {
                coin_list.insert(
                    symbol.to_string(),
                    CoinInfo {
                        symbol: format!("{}-USDT", symbol),
                        name: symbol.to_string(),
                        price: 0.0,
                    },
                );
            }
        }
        //pannel 정의
        let (mut panes, first_pane) = pane_grid::State::new(Pane::Chart);
        let a = panes
            .split(pane_grid::Axis::Vertical, first_pane, Pane::LeftSidebar)
            .unwrap();
        panes.split(pane_grid::Axis::Vertical, a.0, Pane::RightSidebar);

        let (alert_sender, alert_receiver) = mpsc::channel(100);

        Self {
            panes,
            candlesticks: fetch_candles("USDT-BTC", &CandleType::Day, None).unwrap_or_default(),
            selected_coin: "BTC".to_string(),
            selected_candle_type: CandleType::Day,
            coin_list,
            auto_scroll: true,
            ws_sender: None,
            show_ma5: false,
            show_ma10: false,
            show_ma20: false,
            show_ma200: false,
            loading_more: false,
            oldest_date: None,

            account_info: None,
            alerts: VecDeque::with_capacity(5),
            auto_trading_enabled: false,
            last_trade_time: None,
            alert_sender,
            average_prices: HashMap::new(),

            scored_signals_enabled: true, // 기본으로 활성화
            buy_scored_signals: BTreeMap::new(),
            sell_scored_signals: BTreeMap::new(),
        }
    }
}
//Main 메서드
impl Futurx {
    //바이낸스 계정 구독
    fn binance_account_subscription(&self) -> Subscription<Message> {
        Subscription::run(binance_account_connection)
    }
    //전체 구독 설정
    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            // 기존 웹소켓 subscription
            self.websocket_subscription(),
            self.binance_account_subscription(),
            iced::time::every(std::time::Duration::from_millis(100)).map(|_| Message::Tick),
        ])
    }
    //Websocket 구독 설정
    fn websocket_subscription(&self) -> Subscription<Message> {
        Subscription::run(binance_connection)
    }
    //UI
    pub fn view(&self) -> Element<Message> {
        // 패널 그리드 구성
        pane_grid(&self.panes, |pane, content_type, is_maximized| {
            let ma_controls = ma_controls(&self);
            let prediction_display = Container::new(Column::new().push(
                if let Some(alert) = self.alerts.front() {
                    Text::new(&alert.message).color(match alert.alert_type {
                        AlertType::Buy => uc::BRIGH_GREEN,
                        AlertType::Sell => uc::BRIGHT_RED,
                        AlertType::Info => uc::BRIGHT_BLUE,
                        AlertType::Error => uc::BRIGHT_RED,
                    })
                } else {
                    Text::new("")
                },
            ))
            .padding(10)
            .width(Length::Shrink)
            .height(Length::Shrink);

            let coins: Vec<String> = self.coin_list.keys().cloned().collect();
            let coin_picker =
                pick_list(coins, Some(self.selected_coin.clone()), Message::SelectCoin)
                    .width(Length::Fixed(150.0));

            let candle_types = vec![CandleType::Minute1, CandleType::Minute3, CandleType::Day];
            let candle_type_strings: Vec<String> =
                candle_types.iter().map(|ct| ct.to_string()).collect();
            let candle_type_picker = pick_list(
                candle_type_strings,
                Some(self.selected_candle_type.to_string()),
                |s| {
                    let candle_type = match s.as_str() {
                        "1Minute" => CandleType::Minute1,
                        "3Minute" => CandleType::Minute3,
                        "Day" => CandleType::Day,
                        _ => CandleType::Day,
                    };
                    Message::SelectCandleType(candle_type)
                },
            )
            .width(Length::Fixed(100.0));

            match content_type {
                // 차트 패널
                Pane::Chart => {
                    let canvas = Canvas::new(Chart::new(
                        self.candlesticks.clone(),
                        self.selected_candle_type.clone(),
                        self.show_ma5,
                        self.show_ma10,
                        self.show_ma20,
                        self.show_ma200,
                        self.scored_signals_enabled,
                        self.buy_scored_signals.clone(),
                        self.sell_scored_signals.clone(),
                    ))
                    .width(iced::Fill)
                    .height(iced::Fill);

                    // 상단 컨트롤 영역
                    let top_controls = Row::new()
                        .push(coin_picker.width(FillPortion(1)))
                        .push(candle_type_picker.width(FillPortion(1)))
                        .push(ma_controls.width(FillPortion(8)))
                        .push(prediction_display.width(FillPortion(2)));
                    let chart_body = Column::new()
                        .push(Row::new().push(top_controls.width(FillPortion(1))))
                        .push(
                            Row::new().push(container(canvas).width(FillPortion(4))), // .push(container(right_side_bar).width(FillPortion(1))),
                        );
                    pane_grid::Content::new(Container::new(chart_body))
                }

                // 좌측 사이드바 패널 (코인 정보)
                Pane::LeftSidebar => {
                    let coin_info = coin_info(&self);
                    let left_side_bar = Column::new().spacing(20).padding(20).push(coin_info);

                    let title_bar =
                        pane_grid::TitleBar::new(Text::new("코인 정보").size(16)).padding(10);
                    pane_grid::Content::new(left_side_bar).title_bar(title_bar)
                }

                // 우측 사이드바 패널 (계좌 및 거래 정보)
                Pane::RightSidebar => {
                    let auto_trading_toggle = auto_trading_toggle(&self);
                    let account_info = account_info(&self);
                    let order_buttons = order_buttons(&self);
                    let current_position = current_position(&self);

                    let right_side_bar = Column::new()
                        .spacing(20)
                        .padding(20)
                        .push(auto_trading_toggle)
                        .push(account_info)
                        .push(order_buttons)
                        .push(current_position);

                    let title_bar =
                        pane_grid::TitleBar::new(Text::new("거래 정보").size(16)).padding(10);
                    pane_grid::Content::new(right_side_bar).title_bar(title_bar)
                }

                // 기타 패널 유형
                _ => {
                    let title = "utils";
                    let header = pane_grid::TitleBar::new(Text::new(title)).padding(10);
                    let content = Text::new("추가 패널 내용");

                    pane_grid::Content::new(content).title_bar(header)
                }
            }
        })
        .on_drag(Message::PaneDragged)
        .on_resize(10, Message::PaneResized)
        .into()
    }
    pub fn update(&mut self, message: Message) {
        match message {
            Message::PaneDragged(drag_event) => match drag_event {
                pane_grid::DragEvent::Dropped { pane, target } => {
                    if let iced::widget::pane_grid::Target::Pane(dest_pane, _region) = &target {
                        self.panes.swap(pane, *dest_pane);
                    } else {
                        println!("Not")
                    }
                }

                _ => {}
            },
            Message::PaneResized(resize_event) => {
                // 리사이즈 이벤트 처리
                let pane_grid::ResizeEvent { split, ratio } = resize_event;
                println!("분할선 {:?}의 비율이 {:.2}로 변경됨", split, ratio);

                // 분할선 위치 업데이트 로직
                // 예시 코드 (실제 API에 맞게 수정 필요)
                // self.panes.update_ratio(split, ratio);
            } // 다른 메시지 처리
            Message::UpdateAveragePrice(symbol, price) => {
                self.average_prices.insert(symbol, price);
            }
            Message::MarketBuy => market_buy(self),
            Message::MarketSell => market_sell(self),
            Message::ToggleAutoTrading => {
                self.auto_trading_enabled = !self.auto_trading_enabled;
                let status = if self.auto_trading_enabled {
                    "Automatic trading activate"
                } else {
                    "Automatic trading deactivate"
                };
                self.add_alert(format!("{}", status), AlertType::Info);
            }

            Message::TryBuy {
                price,
                strength,
                timestamp,
                indicators,
            } => {
                self.add_alert(
                    format!(
                        "매수 신호 감지!\n가격: {:.2} USDT\n강도: {:.2}\nRSI: {:.2}",
                        price, strength, indicators.rsi
                    ),
                    AlertType::Buy,
                );

                if self.auto_trading_enabled {
                    let can_trade = self
                        .last_trade_time
                        .map(|time| time.elapsed() > Duration::from_secs(60))
                        .unwrap_or(true);

                    if can_trade {
                        let amount = 0.001;
                        let selected_coin = self.selected_coin.clone();
                        let alert_sender = self.alert_sender.clone();

                        let runtime = tokio::runtime::Handle::current();
                        runtime.spawn(async move {
                            if let Err(e) = execute_trade(
                                selected_coin,
                                TradeType::Buy,
                                price,
                                amount,
                                alert_sender,
                            )
                            .await
                            {
                                println!("{}", ul::ORDER_FAIL);
                                println!("매수 실패: {:?}", e);
                            }
                        });

                        self.last_trade_time = Some(Instant::now());
                    }
                }
            }

            Message::TrySell {
                price,
                strength,
                timestamp,
                indicators,
            } => {
                let dt = chrono::DateTime::from_timestamp((timestamp / 1000) as i64, 0)
                    .unwrap_or_default()
                    .with_timezone(&chrono::Local);

                println!("=== 강한 매도 신호 감지! ===");
                println!("시간: {}", dt.format("%Y-%m-%d %H:%M:%S"));
                println!("코인: {}", self.selected_coin);
                println!("가격: {:.2} USDT", price);
                println!("신호 강도: {:.2}", strength);
                println!("RSI: {:.2}", indicators.rsi);
                println!("MA5/MA20: {:.2}/{:.2}", indicators.ma5, indicators.ma20);
                println!("거래량 비율: {:.2}", indicators.volume_ratio);
                println!("========================");

                self.add_alert(
                    format!(
                        "매도 신호 감지!\n가격: {:.2} USDT\n강도: {:.2}\nRSI: {:.2}",
                        price, strength, indicators.rsi
                    ),
                    AlertType::Sell,
                );

                if self.auto_trading_enabled {
                    let can_trade = self
                        .last_trade_time
                        .map(|time| time.elapsed() > Duration::from_secs(60))
                        .unwrap_or(true);

                    if can_trade {
                        let amount = 0.001;
                        let selected_coin = self.selected_coin.clone();
                        let alert_sender = self.alert_sender.clone();

                        let runtime = tokio::runtime::Handle::current();
                        runtime.spawn(async move {
                            if let Err(e) = execute_trade(
                                selected_coin,
                                TradeType::Sell,
                                price,
                                amount,
                                alert_sender,
                            )
                            .await
                            {
                                println!("매도 실패: {:?}", e);
                            }
                        });

                        self.last_trade_time = Some(Instant::now());
                    }
                }
            }
            Message::UpdateAccountInfo(info) => {
                self.account_info = Some(info);
            }

            Message::FetchError(error) => {
                println!("API Error: {}", error);
            }

            Message::AddAlert(message, alert_type) => {
                self.alerts.push_back(Alert {
                    message,
                    alert_type,
                    timestamp: Instant::now(),
                });
            }

            Message::RemoveAlert => {
                //알림제거
                self.alerts.pop_front();
            }

            Message::Tick => {
                // 5초 이상 된 알림 제거
                while let Some(alert) = self.alerts.front() {
                    if alert.timestamp.elapsed() > Duration::from_secs(5) {
                        self.alerts.pop_front();
                    } else {
                        break;
                    }
                }
            }

            Message::LoadMoreCandles => {
                if !self.loading_more {
                    // 가장 오래된 캔들의 날짜를 찾아서 to 파라미터로 사용
                    if let Some((&oldest_timestamp, _)) = self.candlesticks.iter().next() {
                        self.loading_more = true;
                        let datetime = chrono::NaiveDateTime::from_timestamp_opt(
                            (oldest_timestamp / 1000) as i64,
                            0,
                        )
                        .unwrap();
                        let date_str = datetime.format("%Y-%m-%dT%H:%M:%S").to_string();

                        // 클론해서 async 클로저에 전달
                        let market = format!("USDT-{}", self.selected_coin);
                        let candle_type = self.selected_candle_type.clone();

                        let runtime = tokio::runtime::Handle::current();
                        runtime.spawn(async move {
                            match fetch_candles_async(&market, &candle_type, Some(date_str)).await {
                                Ok(new_candles) => Message::MoreCandlesLoaded(new_candles),
                                Err(_) => Message::Error,
                            }
                        });
                    }
                }
            }
            Message::MoreCandlesLoaded(mut new_candles) => {
                if !new_candles.is_empty() {
                    self.candlesticks.append(&mut new_candles);
                }
            }

            //이동평슌선 5,10,20,200일선
            Message::ToggleMA5 => self.show_ma5 = !self.show_ma5,
            Message::ToggleMA10 => self.show_ma10 = !self.show_ma10,
            Message::ToggleMA20 => self.show_ma20 = !self.show_ma20,
            Message::ToggleMA200 => self.show_ma200 = !self.show_ma200,
            Message::SelectCandleType(candle_type) => {
                println!("Changing candle type to: {}", candle_type);
                self.selected_candle_type = candle_type.clone();

                // 캔들스틱 데이터 새로 불러오기
                let market = format!("USDT-{}", self.selected_coin);
                println!(
                    "Fetching new candles for market {} with type {}",
                    market, candle_type
                );

                match fetch_candles(&market, &candle_type, None) {
                    // None을 추가하여 최신 데이터부터 가져오기
                    Ok(candles) => {
                        println!(
                            "Successfully fetched {} candles for {}",
                            candles.len(),
                            candle_type
                        );
                        self.candlesticks = candles;

                        // 가장 오래된 캔들의 날짜 저장
                        if let Some((&timestamp, _)) = self.candlesticks.iter().next() {
                            let datetime = chrono::NaiveDateTime::from_timestamp_opt(
                                (timestamp / 1000) as i64,
                                0,
                            )
                            .unwrap();
                            self.oldest_date =
                                Some(datetime.format("%Y-%m-%dT%H:%M:%S").to_string());
                        } else {
                            self.oldest_date = None;
                        }

                        self.auto_scroll = true;
                    }
                    Err(e) => {
                        println!("Error fetching {} candles: {:?}", candle_type, e);
                    }
                }
            }
            Message::UpdatePrice(symbol, price, change_rate) => {
                if let Some(info) = self.coin_list.get_mut(&symbol) {
                    info.price = price;
                }
            }
            Message::WebSocketInit(sender) => {
                println!("WebSocket sender initialized!");
                self.ws_sender = Some(sender);
            }
            Message::SelectCoin(symbol) => {
                println!("Switching to coin: {}", symbol);
                self.selected_coin = symbol.clone();

                if let Some(sender) = &self.ws_sender {
                    println!("Sending WebSocket subscription for: {}", symbol);
                    if let Err(e) = sender.clone().try_send(symbol.clone()) {
                        println!("ERROR sending WebSocket subscription: {:?}", e);
                    } else {
                        println!("WebSocket subscription sent successfully");
                    }
                } else {
                    println!("ERROR: WebSocket sender is None!");
                }
                self.candlesticks.clear();

                match fetch_candles(
                    &format!("USDT-{}", symbol),
                    &self.selected_candle_type,
                    None,
                ) {
                    Ok(candles) => {
                        if candles.is_empty() {
                            println!("Warning: No candles received for {}", symbol);
                        } else {
                            println!(
                                "Successfully loaded {} candles for {}",
                                candles.len(),
                                symbol
                            );
                            self.candlesticks = candles;

                            // 가장 오래된 캔들의 날짜 저장
                            if let Some((&timestamp, _)) = self.candlesticks.iter().next() {
                                let datetime = chrono::NaiveDateTime::from_timestamp_opt(
                                    (timestamp / 1000) as i64,
                                    0,
                                )
                                .unwrap();
                                self.oldest_date =
                                    Some(datetime.format("%Y-%m-%dT%H:%M:%S").to_string());
                            }
                        }
                    }
                    Err(e) => {
                        println!("Error fetching candles for {}: {:?}", symbol, e);
                    }
                }

                if let Some(sender) = &self.ws_sender {
                    if let Err(e) = sender.clone().try_send(symbol.clone()) {
                        println!("Error sending WebSocket subscription: {:?}", e);
                    }
                }
                self.auto_scroll = true;
            }
            Message::UpdateCoinPrice(symbol, price, change) => {
                if let Some(info) = self.coin_list.get_mut(&symbol) {
                    info.price = price;
                }
            }
            Message::AddCandlestick(trade) => {
                let (timestamp, trade_data) = trade;
                let current_market = format!("{}USDT", self.selected_coin);

                if trade_data.symbol != current_market {
                    return;
                }

                if self.scored_signals_enabled {
                    println!("📊 Scored signals enabled! Calculating...");
                    println!("📊 Candlesticks count: {}", self.candlesticks.len());

                    let (buy_scores, sell_scores) = calculate_scored_signals(
                        &self.candlesticks,
                        true,
                        &self.selected_candle_type,
                    );

                    println!(
                        "📊 Calculated buy_scores: {}, sell_scores: {}",
                        buy_scores.len(),
                        sell_scores.len()
                    );

                    // 계산된 점수들 출력
                    for (timestamp, score) in &buy_scores {
                        println!("📊 Buy score at {}: {:.1}", timestamp, score.total_score);
                    }
                    for (timestamp, score) in &sell_scores {
                        println!("📊 Sell score at {}: {:.1}", timestamp, score.total_score);
                    }

                    self.buy_scored_signals = buy_scores;
                    self.sell_scored_signals = sell_scores;
                } else {
                    println!("📊 Scored signals DISABLED");
                }

                if self.candlesticks.is_empty() {
                    // 초기 데이터 로드
                    if let Ok(candles) = fetch_candles(
                        &format!("USDT-{}", self.selected_coin),
                        &self.selected_candle_type,
                        None,
                    ) {
                        self.candlesticks = candles;
                    }
                }
                if self.scored_signals_enabled {
                    let (buy_scores, sell_scores) = calculate_scored_signals(
                        &self.candlesticks,
                        true,
                        &self.selected_candle_type,
                    );

                    self.buy_scored_signals = buy_scores;
                    self.sell_scored_signals = sell_scores;

                    // 최신 신호 확인
                    if let Some(&last_timestamp) = self.candlesticks.keys().last() {
                        if let Some(buy_score) = self.buy_scored_signals.get(&last_timestamp) {
                            if buy_score.total_score >= 85.0 {
                                self.add_alert(
                                    format!(
                                        "초강력 매수 신호! 점수: {:.0}/100",
                                        buy_score.total_score
                                    ),
                                    AlertType::Buy,
                                );
                            }
                        }

                        if let Some(sell_score) = self.sell_scored_signals.get(&last_timestamp) {
                            if sell_score.total_score >= 85.0 {
                                self.add_alert(
                                    format!(
                                        "초강력 매도 신호! 점수: {:.0}/100",
                                        sell_score.total_score
                                    ),
                                    AlertType::Sell,
                                );
                            }
                        }
                    }
                }
                let current_timestamp = timestamp;
                let candle_timestamp = match self.selected_candle_type {
                    CandleType::Minute1 => current_timestamp - (current_timestamp % 60000),
                    CandleType::Minute3 => current_timestamp - (current_timestamp % 180000),
                    CandleType::Day => current_timestamp - (current_timestamp % 86400000),
                };

                let trade_price = trade_data.price.parse::<f32>().unwrap_or_default();
                let trade_volume = trade_data.quantity.parse::<f32>().unwrap_or_default();

                self.candlesticks
                    .entry(candle_timestamp)
                    .and_modify(|candle| {
                        candle.high = candle.high.max(trade_price);
                        candle.low = candle.low.min(trade_price);
                        candle.close = trade_price;
                        candle.volume += trade_volume;
                    })
                    .or_insert(Candlestick {
                        open: trade_price,
                        high: trade_price,
                        low: trade_price,
                        close: trade_price,
                        volume: trade_volume,
                    });
                self.auto_scroll = true;
            }
            Message::RemoveCandlestick => {
                if let Some(&last_key) = self.candlesticks.keys().last() {
                    self.candlesticks.remove(&last_key);
                }
                self.auto_scroll = true;
            }

            Message::Error => {
                println!("WebSocket connection error");
            }
            Message::ToggleScoredSignals => {
                self.scored_signals_enabled = !self.scored_signals_enabled;
                if !self.scored_signals_enabled {
                    self.buy_scored_signals.clear();
                    self.sell_scored_signals.clear();
                }
            }
        }
    }

    fn add_alert(&mut self, message: String, alert_type: AlertType) {
        self.alerts.push_back(Alert {
            message,
            alert_type,
            timestamp: Instant::now(),
        });

        // 최대 5개까지만 유지
        while self.alerts.len() > 5 {
            self.alerts.pop_front();
        }
    }
}

impl std::fmt::Display for CandleType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CandleType::Minute1 => write!(f, "1Minute"),
            CandleType::Minute3 => write!(f, "3Minute"), // 표시 텍스트 변경
            CandleType::Day => write!(f, "Day"),
        }
    }
}
fn main() -> iced::Result {
    //환경변수 설정
    dotenv().ok();
    iced::application("Futurx", Futurx::update, Futurx::view)
        .subscription(Futurx::subscription)
        .window_size(Size::new(1900., 1020.))
        .run()
}
