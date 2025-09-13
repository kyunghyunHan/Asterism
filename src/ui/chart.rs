use crate::utils::constant as uc;
use crate::{CandleType, Candlestick, Chart, ChartState};
use iced::{
    mouse,
    widget::{
        canvas,
        canvas::{
            event::{self, Event},
            Program,
        },
    },
    Color, Pixels, Point, Rectangle, Size,
};
use std::collections::{BTreeMap, VecDeque};

pub fn calculate_rsi(
    candlesticks: &BTreeMap<u64, Candlestick>,
    period: usize,
) -> BTreeMap<u64, f32> {
    let mut rsi_values = BTreeMap::new();
    if candlesticks.len() < period + 1 {
        return rsi_values;
    }

    let mut gains = Vec::new();
    let mut losses = Vec::new();
    let mut prev_close = None;
    let mut timestamps = Vec::new();

    // 가격 변화 계산
    for (timestamp, candle) in candlesticks.iter() {
        if let Some(prev) = prev_close {
            let change = candle.close - prev;
            timestamps.push(*timestamp);
            if change >= 0.0 {
                gains.push(change);
                losses.push(0.0);
            } else {
                gains.push(0.0);
                losses.push(-change);
            }
        }
        prev_close = Some(candle.close);
    }

    // RSI 계산
    for i in period..timestamps.len() {
        let avg_gain: f32 = gains[i - period..i].iter().sum::<f32>() / period as f32;
        let avg_loss: f32 = losses[i - period..i].iter().sum::<f32>() / period as f32;

        let rs = if avg_loss == 0.0 {
            100.0
        } else {
            avg_gain / avg_loss
        };

        let rsi = 100.0 - (100.0 / (1.0 + rs));
        rsi_values.insert(timestamps[i], rsi);
    }

    rsi_values
}

pub fn calculate_moving_average(
    candlesticks: &BTreeMap<u64, Candlestick>,
    period: usize,
) -> BTreeMap<u64, f32> {
    let mut result = BTreeMap::new();
    if period == 0 || candlesticks.is_empty() {
        return result;
    }

    let data: Vec<(&u64, &Candlestick)> = candlesticks.iter().collect();

    // 모든 캔들에 대해 이동평균 계산
    for i in 0..data.len() {
        if i >= period - 1 {
            let sum: f32 = data[i + 1 - period..=i]
                .iter()
                .map(|(_, candle)| candle.close)
                .sum();
            let avg = sum / period as f32;
            result.insert(*data[i].0, avg);
        }
    }

    result
}

impl Chart {
    pub fn new(
        candlesticks: BTreeMap<u64, Candlestick>,
        candle_type: CandleType,
        show_ma5: bool,
        show_ma10: bool,
        show_ma20: bool,
        show_ma200: bool,
        scored_signals_enabled: bool,
        buy_scored_signals: BTreeMap<u64, SignalScoring>,
        sell_scored_signals: BTreeMap<u64, SignalScoring>,
    ) -> Self {
        let ma5_values = calculate_moving_average(&candlesticks, 5);
        let ma10_values = calculate_moving_average(&candlesticks, 10);
        let ma20_values = calculate_moving_average(&candlesticks, 20);
        let ma200_values = calculate_moving_average(&candlesticks, 200);
        let rsi_values = calculate_rsi(&candlesticks, 14);

        let price_range = if candlesticks.is_empty() {
            Some((0.0, 100.0))
        } else {
            let (min, max) = candlesticks.values().fold((f32::MAX, f32::MIN), |acc, c| {
                (acc.0.min(c.low), acc.1.max(c.high))
            });

            let ma_min = [&ma5_values, &ma10_values, &ma20_values, &ma200_values]
                .iter()
                .filter(|ma| !ma.is_empty())
                .flat_map(|ma| ma.values())
                .fold(min, |acc, &x| acc.min(x));

            let ma_max = [&ma5_values, &ma10_values, &ma20_values, &ma200_values]
                .iter()
                .filter(|ma| !ma.is_empty())
                .flat_map(|ma| ma.values())
                .fold(max, |acc, &x| acc.max(x));

            let margin = (ma_max - ma_min) * 0.1;
            Some((ma_min - margin, ma_max + margin))
        };
        let max_data_points = 1000; // 저장할 최대 데이터 수
        let mut candlestick_deque: VecDeque<(u64, Candlestick)> =
            VecDeque::with_capacity(max_data_points);

        // 정렬된 데이터를 VecDeque에 추가
        for (timestamp, candle) in candlesticks.into_iter() {
            if candlestick_deque.len() >= max_data_points {
                candlestick_deque.pop_front(); // 가장 오래된 데이터 제거
            }
            candlestick_deque.push_back((timestamp, candle));
        }

        Self {
            candlesticks: candlestick_deque,
            max_data_points,
            state: ChartState {
                auto_scroll: true,
                ..ChartState::default()
            },
            price_range,
            candle_type,
            show_ma5,
            show_ma10,
            show_ma20,
            show_ma200,
            ma5_values,
            ma10_values,
            ma20_values,
            ma200_values,
            rsi_values,
            show_rsi: true,
            scored_signals_enabled,
            buy_scored_signals,
            sell_scored_signals,
        }
    }
}
impl<Message> Program<Message> for Chart {
    type State = ChartState;

    fn update(
        &self,
        state: &mut Self::State,
        event: Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> (event::Status, Option<Message>) {
        let cursor_position = if let Some(position) = cursor.position() {
            position
        } else {
            return (event::Status::Ignored, None);
        };

        match event {
            Event::Mouse(mouse_event) => match mouse_event {
                mouse::Event::ButtonPressed(mouse::Button::Left) => {
                    state.dragging = true;
                    state.drag_start = cursor_position;
                    state.last_offset = state.offset;
                    state.auto_scroll = false;
                    (event::Status::Captured, None)
                }
                mouse::Event::ButtonReleased(mouse::Button::Left) => {
                    state.dragging = false;
                    (event::Status::Captured, None)
                }
                mouse::Event::CursorMoved { .. } => {
                    if state.dragging {
                        let delta_x = cursor_position.x - state.drag_start.x; // 드래그 방향과 크기
                        let new_offset = state.last_offset + delta_x;
                        // println!("{}", cursor_position.x);
                        // 드래그가 좌로 이동했을 때 처리 (delta_x < 0)
                        if delta_x < 0.0 && new_offset < state.offset && !state.need_more_data {
                            // println!("{}", "좌로 드래그 - 이전 데이터 로드 필요");

                            state.need_more_data = true; // 데이터를 요청해야 한다는 플래그 설정
                        }

                        // 새로운 오프셋 업데이트
                        state.offset = new_offset;
                        (event::Status::Captured, None)
                    } else {
                        (event::Status::Ignored, None)
                    }
                }
                _ => (event::Status::Ignored, None),
            },
            _ => (event::Status::Ignored, None),
        }
    }
    fn draw(
        &self,
        state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        if self.candlesticks.is_empty() {
            return vec![frame.into_geometry()];
        }

        // 여백 설정
        let left_margin = 50.0;
        let right_margin = 20.0;
        let top_margin = 20.0;
        let bottom_margin = 50.0;

        // 차트 영역 설정
        let price_chart_height = bounds.height * 0.5;
        let volume_height = 100.0;
        let rsi_height = 80.0;
        let charts_gap = 20.0;
        let margin = 20.0;

        let remaining_height = bounds.height - price_chart_height - margin - bottom_margin;
        let volume_area_height = remaining_height * 0.5;
        let rsi_area_height = remaining_height * 0.4;

        let price_area_end = margin + price_chart_height;
        let volume_area_start = price_area_end + charts_gap;
        let volume_area_end = volume_area_start + volume_area_height;
        let rsi_area_start = volume_area_end + charts_gap;
        let rsi_area_end = bounds.height - bottom_margin;

        // 배경 그리기
        frame.fill_rectangle(
            Point::new(0.0, 0.0),
            bounds.size(),
            Color::from_rgb(0.1, 0.1, 0.15),
        );

        // 가격 범위 계산
        let (mut min_price, mut max_price) = self
            .candlesticks
            .iter()
            .fold((f32::MAX, f32::MIN), |acc, (_, c)| {
                (acc.0.min(c.low), acc.1.max(c.high))
            });

        // 여유 공간 추가
        let price_margin = (max_price - min_price) * 0.1;
        min_price = (min_price - price_margin).max(0.0);
        max_price += price_margin;

        // 거래량 최대값 계산
        let max_volume = self
            .candlesticks
            .iter()
            .map(|(_, c)| c.volume)
            .fold(0.0, f32::max);

        // 캔들스틱 크기 계산
        let available_width = bounds.width - left_margin - right_margin;
        let candles_per_screen = 1000;
        let base_candle_width = match self.candle_type {
            CandleType::Minute1 => 10.0,
            CandleType::Minute3 => 10.0,
            CandleType::Day => 10.0,
        };
        let target_position = available_width * 0.95;
        let total_chart_width = candles_per_screen as f32 * base_candle_width;
        let initial_offset = target_position - total_chart_width;

        let body_width = base_candle_width * 0.8;

        // 스케일링 계산
        let price_diff = (max_price - min_price).max(f32::EPSILON);
        let y_scale = (price_chart_height / price_diff).min(1e6);
        let volume_scale = (volume_height / max_volume).min(1e6);
        let price_format = |price: f32| {
            if price < 0.0001 {
                format!("{:.8}", price) // 매우 작은 가격
            } else if price < 0.01 {
                format!("{:.6}", price) // 작은 가격
            } else if price < 1.0 {
                format!("{:.4}", price) // 중간 가격
            } else {
                format!("{:.2}", price) // 큰 가격
            }
        };
        // 가격 차트 그리드 라인
        for i in 0..=10 {
            let y = top_margin + (price_chart_height * (i as f32 / 10.0));
            let price = max_price - (price_diff * (i as f32 / 10.0));

            frame.stroke(
                &canvas::Path::new(|p| {
                    p.move_to(Point::new(left_margin, y));
                    p.line_to(Point::new(bounds.width - right_margin, y));
                }),
                canvas::Stroke::default()
                    .with_color(Color::from_rgb(0.2, 0.2, 0.25))
                    .with_width(1.0),
            );

            frame.fill_text(canvas::Text {
                content: price_format(price),
                position: Point::new(5.0, y - 5.0),
                color: Color::from_rgb(0.7, 0.7, 0.7),
                size: Pixels(10.0),
                ..canvas::Text::default()
            });
        }

        // 현재 스크롤 위치 계산
        let scroll_offset = (-state.offset / base_candle_width) as usize;

        // visible_candlesticks 생성
        let visible_candlesticks: Vec<(u64, &Candlestick)> = self
            .candlesticks
            .iter()
            .skip(scroll_offset)
            .take(candles_per_screen)
            .map(|(ts, candle)| (*ts, candle))
            .collect();
        // visible_candlesticks 그리기 이후에 다음 코드 추가

        // 이동평균선 그리기
        if self.show_ma5 {
            let ma_points: Vec<Point> = visible_candlesticks
                .iter()
                .enumerate()
                .filter_map(|(i, (ts, _))| {
                    self.ma5_values.get(ts).map(|&ma| {
                        Point::new(
                            left_margin
                                + (i as f32 * base_candle_width)
                                + initial_offset
                                + state.offset,
                            top_margin + ((max_price - ma) * y_scale),
                        )
                    })
                })
                .collect();

            if ma_points.len() >= 2 {
                frame.stroke(
                    &canvas::Path::new(|p| {
                        p.move_to(ma_points[0]);
                        for point in ma_points.iter().skip(1) {
                            p.line_to(*point);
                        }
                    }),
                    canvas::Stroke::default()
                        .with_color(uc::ORNAGE) // 주황색
                        .with_width(1.0),
                );
            }
        }

        if self.show_ma10 {
            let ma_points: Vec<Point> = visible_candlesticks
                .iter()
                .enumerate()
                .filter_map(|(i, (ts, _))| {
                    self.ma10_values.get(ts).map(|&ma| {
                        Point::new(
                            left_margin
                                + (i as f32 * base_candle_width)
                                + initial_offset
                                + state.offset,
                            top_margin + ((max_price - ma) * y_scale),
                        )
                    })
                })
                .collect();

            if ma_points.len() >= 2 {
                frame.stroke(
                    &canvas::Path::new(|p| {
                        p.move_to(ma_points[0]);
                        for point in ma_points.iter().skip(1) {
                            p.line_to(*point);
                        }
                    }),
                    canvas::Stroke::default()
                        .with_color(uc::YELLOW) // 노란색
                        .with_width(1.0),
                );
            }
        }

        if self.show_ma20 {
            let ma_points: Vec<Point> = visible_candlesticks
                .iter()
                .enumerate()
                .filter_map(|(i, (ts, _))| {
                    self.ma20_values.get(ts).map(|&ma| {
                        Point::new(
                            left_margin
                                + (i as f32 * base_candle_width)
                                + initial_offset
                                + state.offset,
                            top_margin + ((max_price - ma) * y_scale),
                        )
                    })
                })
                .collect();

            if ma_points.len() >= 2 {
                frame.stroke(
                    &canvas::Path::new(|p| {
                        p.move_to(ma_points[0]);
                        for point in ma_points.iter().skip(1) {
                            p.line_to(*point);
                        }
                    }),
                    canvas::Stroke::default()
                        .with_color(uc::DAKR_RED) // 빨간색
                        .with_width(1.0),
                );
            }
        }

        if self.show_ma200 {
            let ma_points: Vec<Point> = visible_candlesticks
                .iter()
                .enumerate()
                .filter_map(|(i, (ts, _))| {
                    self.ma200_values.get(ts).map(|&ma| {
                        Point::new(
                            left_margin
                                + (i as f32 * base_candle_width)
                                + initial_offset
                                + state.offset,
                            top_margin + ((max_price - ma) * y_scale),
                        )
                    })
                })
                .collect();

            if ma_points.len() >= 2 {
                frame.stroke(
                    &canvas::Path::new(|p| {
                        p.move_to(ma_points[0]);
                        for point in ma_points.iter().skip(1) {
                            p.line_to(*point);
                        }
                    }),
                    canvas::Stroke::default()
                        .with_color(Color::from_rgb(0.0, 0.0, 1.0)) // 파란색
                        .with_width(1.0),
                );
            }
        }

        // RSI 그리기
        if self.show_rsi {
            // RSI 영역 그리드 라인
            for i in 0..=4 {
                let y = rsi_area_start + (rsi_area_height * (i as f32 / 4.0));
                let rsi_value = 100.0 - (100.0 * (i as f32 / 4.0));

                frame.stroke(
                    &canvas::Path::new(|p| {
                        p.move_to(Point::new(left_margin, y));
                        p.line_to(Point::new(bounds.width - right_margin, y));
                    }),
                    canvas::Stroke::default()
                        .with_color(Color::from_rgb(0.2, 0.2, 0.25))
                        .with_width(1.0),
                );

                frame.fill_text(canvas::Text {
                    content: format!("RSI {:.0}", rsi_value),
                    position: Point::new(5.0, y - 5.0),
                    color: Color::from_rgb(0.7, 0.7, 0.7),
                    size: Pixels(10.0),
                    ..canvas::Text::default()
                });
            }

            // RSI 선 그리기
            let rsi_points: Vec<Point> = visible_candlesticks
                .iter()
                .enumerate()
                .filter_map(|(i, (ts, _))| {
                    self.rsi_values.get(ts).map(|&rsi| {
                        Point::new(
                            left_margin
                                + (i as f32 * base_candle_width)
                                + initial_offset
                                + state.offset,
                            rsi_area_start + (rsi_area_height * (1.0 - rsi / 100.0)),
                        )
                    })
                })
                .collect();

            if rsi_points.len() >= 2 {
                frame.stroke(
                    &canvas::Path::new(|p| {
                        p.move_to(rsi_points[0]);
                        for point in rsi_points.iter().skip(1) {
                            p.line_to(*point);
                        }
                    }),
                    canvas::Stroke::default()
                        .with_color(Color::from_rgb(0.0, 0.8, 0.8)) // 청록색
                        .with_width(1.0),
                );
            }
        }
        // 캔들스틱과 거래량 바 그리기
        for (i, (ts, candlestick)) in visible_candlesticks.iter().enumerate() {
            let x = left_margin + (i as f32 * base_candle_width) + initial_offset + state.offset;

            let color = if candlestick.close >= candlestick.open {
                Color::from_rgb(0.8, 0.0, 0.0)
            } else {
                Color::from_rgb(0.0, 0.0, 0.8)
            };

            let open_y = top_margin + ((max_price - candlestick.open) * y_scale);
            let close_y = top_margin + ((max_price - candlestick.close) * y_scale);
            let high_y = top_margin + ((max_price - candlestick.high) * y_scale);
            let low_y = top_margin + ((max_price - candlestick.low) * y_scale);

            // 심지
            let center_x = x + (body_width / 2.0);
            frame.stroke(
                &canvas::Path::new(|builder| {
                    builder.move_to(Point::new(center_x, high_y));
                    builder.line_to(Point::new(center_x, low_y));
                }),
                canvas::Stroke::default().with_color(color).with_width(1.0),
            );

            // 캔들 몸통
            let body_height = (close_y - open_y).abs().max(1.0);
            let body_y = close_y.min(open_y);
            frame.fill_rectangle(
                Point::new(x, body_y),
                Size::new(body_width, body_height),
                color,
            );

            // 거래량 바
            let volume_height = candlestick.volume * volume_scale;
            let volume_color = if candlestick.close >= candlestick.open {
                Color::from_rgba(0.8, 0.0, 0.0, 0.5)
            } else {
                Color::from_rgba(0.0, 0.0, 0.8, 0.5)
            };

            frame.fill_rectangle(
                Point::new(x, volume_area_end),
                Size::new(body_width, -volume_height),
                volume_color,
            );

            // 시간 레이블
            if i % 10 == 0 {
                let time_str = match self.candle_type {
                    CandleType::Minute1 | CandleType::Minute3 => {
                        let dt = chrono::DateTime::from_timestamp((*ts / 1000) as i64, 0)
                            .unwrap_or_default()
                            .with_timezone(&chrono::Local);
                        dt.format("%H:%M").to_string()
                    }
                    CandleType::Day => {
                        let dt = chrono::DateTime::from_timestamp((*ts / 1000) as i64, 0)
                            .unwrap_or_default()
                            .with_timezone(&chrono::Local);
                        dt.format("%m/%d").to_string()
                    }
                };

                frame.fill_text(canvas::Text {
                    content: time_str,
                    position: Point::new(x - 15.0, bounds.height - bottom_margin + 15.0),
                    color: Color::from_rgb(0.7, 0.7, 0.7),
                    size: Pixels(10.0),
                    ..canvas::Text::default()
                });
            }
            // println!("Buy scored signals count: {}", self.buy_scored_signals.len());
            // println!("Sell scored signals count: {}", self.sell_scored_signals.len());

            if let Some(buy_score) = self.buy_scored_signals.get(ts) {
                let signal_y = top_margin + ((max_price - candlestick.low) * y_scale) + 45.0;
                let center_x = x + body_width / 2.0;

                // 점수에 따른 색상 강도
                let alpha = (buy_score.total_score / 100.0) * 0.8 + 0.2;
                let color = Color::from_rgba(0., 255., 100., (alpha * 255.0) as f32);

                // 큰 상승 화살표
                let arrow_size = 8.0 + (buy_score.total_score - 70.0) / 30.0 * 4.0;

                frame.fill(
                    &canvas::Path::new(|p| {
                        p.move_to(Point::new(center_x - arrow_size, signal_y));
                        p.line_to(Point::new(center_x, signal_y - arrow_size * 1.5));
                        p.line_to(Point::new(center_x + arrow_size, signal_y));
                        p.line_to(Point::new(center_x + arrow_size * 0.5, signal_y));
                        p.line_to(Point::new(
                            center_x + arrow_size * 0.5,
                            signal_y + arrow_size,
                        ));
                        p.line_to(Point::new(
                            center_x - arrow_size * 0.5,
                            signal_y + arrow_size,
                        ));
                        p.line_to(Point::new(center_x - arrow_size * 0.5, signal_y));
                        p.close();
                    }),
                    color,
                );

                // 점수 텍스트 표시
                frame.fill_text(canvas::Text {
                    content: format!("{:.0}", buy_score.total_score),
                    position: Point::new(center_x - 10.0, signal_y + arrow_size + 15.0),
                    color: Color::WHITE,
                    size: Pixels(10.0),
                    ..canvas::Text::default()
                });
            }

            // 점수 기반 매도 신호
            if let Some(sell_score) = self.sell_scored_signals.get(ts) {
                let signal_y = top_margin + ((max_price - candlestick.high) * y_scale) - 45.0;
                let center_x = x + body_width / 2.0;

                let alpha = (sell_score.total_score / 100.0) * 0.8 + 0.2;
                let color = Color::from_rgba(255., 50., 50., (alpha * 255.0) as f32);

                let arrow_size = 8.0 + (sell_score.total_score - 70.0) / 30.0 * 4.0;

                // 하향 화살표
                frame.fill(
                    &canvas::Path::new(|p| {
                        p.move_to(Point::new(
                            center_x - arrow_size * 0.5,
                            signal_y - arrow_size,
                        ));
                        p.line_to(Point::new(
                            center_x + arrow_size * 0.5,
                            signal_y - arrow_size,
                        ));
                        p.line_to(Point::new(center_x + arrow_size * 0.5, signal_y));
                        p.line_to(Point::new(center_x + arrow_size, signal_y));
                        p.line_to(Point::new(center_x, signal_y + arrow_size * 1.5));
                        p.line_to(Point::new(center_x - arrow_size, signal_y));
                        p.close();
                    }),
                    color,
                );

                frame.fill_text(canvas::Text {
                    content: format!("{:.0}", sell_score.total_score),
                    position: Point::new(center_x - 10.0, signal_y - arrow_size - 5.0),
                    color: Color::WHITE,
                    size: Pixels(10.0),
                    ..canvas::Text::default()
                });
            }
        }

        vec![frame.into_geometry()]
    }
}
// ui/chart.rs에 추가
use crate::models::{CandlestickPatterns, SignalScoring};

pub fn calculate_scored_signals(
    candlesticks: &BTreeMap<u64, Candlestick>,
    is_realtime: bool,
    candle_type: &CandleType,
) -> (BTreeMap<u64, SignalScoring>, BTreeMap<u64, SignalScoring>) {
    let mut buy_scores = BTreeMap::new();
    let mut sell_scores = BTreeMap::new();

    let data: Vec<(&u64, &Candlestick)> = candlesticks.iter().collect();
    let window_size = 20;

    if data.len() < window_size {
        return (buy_scores, sell_scores);
    }

    for i in window_size..data.len() {
        let (timestamp, _candle) = data[i];
        let mut scoring = SignalScoring::new();

        // 3. 상승 포용선 점수 (0-25점)
        scoring.bullish_engulfing = CandlestickPatterns::detect_bullish_engulfing(&data, i);

        // 4. 샛별 점수 (0-25점)
        scoring.morning_star = CandlestickPatterns::detect_morning_star(&data, i);

        // 매수 총점 계산
        let buy_total = scoring.bullish_engulfing + scoring.morning_star;
        scoring.total_score = buy_total;

        // 70점 이상일 때만 신호 발생
        if buy_total >= 70.0 {
            buy_scores.insert(*timestamp, scoring.clone());

            if is_realtime && i == data.len() - 1 {
                println!("🔥 강력한 매수 신호! 총점: {:.1}/100", buy_total);
                println!(
                    "  🟢 Bullish Engulfing: {:.1}/25",
                    scoring.bullish_engulfing
                );
                println!("  ⭐ Morning Star: {:.1}/25", scoring.morning_star);
                println!("========================");
            }
        }

        // 매도 신호도 동일하게 계산
        let mut sell_scoring = SignalScoring::new();

        sell_scoring.bearish_engulfing = CandlestickPatterns::detect_bearish_engulfing(&data, i);
        sell_scoring.evening_star = CandlestickPatterns::detect_evening_star(&data, i);

        let sell_total = sell_scoring.bearish_engulfing + sell_scoring.evening_star;
        sell_scoring.total_score = sell_total;

        if sell_total >= 70.0 {
            sell_scores.insert(*timestamp, sell_scoring.clone());

            if is_realtime && i == data.len() - 1 {
                println!("🔥 강력한 매도 신호! 총점: {:.1}/100", sell_total);

                println!(
                    "  🔴 Bearish Engulfing: {:.1}/25",
                    sell_scoring.bearish_engulfing
                );
                println!("  🌟 Evening Star: {:.1}/25", sell_scoring.evening_star);
                println!("========================");
            }
        }
    }

    (buy_scores, sell_scores)
}
