use crate::Futurx;
use crate::Message;
use iced::widget::{checkbox, Column, Container, Row};

//지표
pub fn ma_controls(r: &Futurx) -> Container<'static, Message> {
    let ma_controls = Container::new(
        Column::new()
            .spacing(5)
            .push(
                Row::new()
                    .spacing(10)
                    .push(checkbox("MA5", r.show_ma5).on_toggle(|_| Message::ToggleMA5))
                    .push(checkbox("MA10", r.show_ma10).on_toggle(|_| Message::ToggleMA10)),
            )
            .push(
                Row::new()
                    .spacing(10)
                    .push(checkbox("MA20", r.show_ma20).on_toggle(|_| Message::ToggleMA20))
                    .push(checkbox("MA200", r.show_ma200).on_toggle(|_| Message::ToggleMA200)),
            ),
    )
    .padding(10);
    ma_controls
}
