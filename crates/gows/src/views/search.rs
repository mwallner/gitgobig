use iced::widget::{button, checkbox, container, row, text, text_input};
use iced::{color, Element, Fill, Length};

use crate::app::App;
use crate::message::Message;

impl App {
    pub(crate) fn view_search_bar(&self) -> Element<'_, Message> {
        let depth_input = text_input("commits…", &self.search_depth)
            .on_input(Message::SearchDepth)
            .size(12)
            .width(Length::Fixed(80.0))
            .padding(4);

        let hash_input = text_input("hash…", &self.search_hash)
            .on_input(Message::SearchHash)
            .size(12)
            .width(Length::Fixed(self.hash_width))
            .padding(4);

        let msg_input = text_input("message…", &self.search_message)
            .on_input(Message::SearchMessage)
            .size(12)
            .width(Fill)
            .padding(4);

        let date_input = text_input("date…", &self.search_date)
            .on_input(Message::SearchDate)
            .size(12)
            .width(Length::Fixed(self.date_width))
            .padding(4);

        let author_input = text_input("author…", &self.search_author)
            .on_input(Message::SearchAuthor)
            .size(12)
            .width(Length::Fixed(self.author_width))
            .padding(4);

        let regex_cb = checkbox(self.search_regex)
            .label("Regex")
            .on_toggle(Message::ToggleRegex)
            .size(14)
            .text_size(12);

        let clear_btn = button(text("Clear").size(11))
            .on_press(Message::ClearSearch)
            .padding([2, 8]);

        container(
            row![
                depth_input,
                hash_input,
                msg_input,
                date_input,
                author_input,
                regex_cb,
                clear_btn,
            ]
            .spacing(4)
            .padding([4, 8])
            .align_y(iced::Alignment::Center),
        )
        .style(|_theme| container::Style {
            background: Some(color!(0x1e1e2e).into()),
            border: iced::Border {
                color: color!(0x45475a),
                width: 0.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
    }
}
