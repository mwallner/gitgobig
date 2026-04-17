use iced::widget::{
    button, center, column, container, mouse_area, opaque, scrollable, text,
};
use iced::{color, Element, Fill, Font, Length};

use crate::app::App;
use crate::message::Message;
use crate::style::{ContextMenu, InspectState};

impl App {
    pub(crate) fn view_context_menu<'a>(&self, cm: &ContextMenu) -> Element<'a, Message> {
        let commit = &self.commits[cm.commit_index];
        let hash = commit.hash.clone();
        let hash2 = commit.hash.clone();

        let menu = container(
            column![
                text(format!("Commit {}", &commit.short_hash))
                    .size(13)
                    .color(color!(0x89b4fa)),
                button(text("Copy commit hash").size(13))
                    .on_press(Message::CopyHash(hash))
                    .width(Fill)
                    .padding(6),
                button(text("Inspect").size(13))
                    .on_press(Message::InspectCommit(hash2))
                    .width(Fill)
                    .padding(6),
            ]
            .spacing(4)
            .padding(8),
        )
        .style(|_theme| container::Style {
            background: Some(color!(0x313244).into()),
            border: iced::Border {
                color: color!(0x585878),
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow: iced::Shadow {
                color: color!(0x000000),
                offset: iced::Vector::new(2.0, 2.0),
                blur_radius: 10.0,
            },
            ..Default::default()
        })
        .max_width(220);

        opaque(
            mouse_area(
                center(opaque(menu))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(|_theme| container::Style {
                        background: Some(
                            iced::Color {
                                a: 0.3,
                                ..color!(0x000000)
                            }
                            .into(),
                        ),
                        ..Default::default()
                    }),
            )
            .on_press(Message::DismissContextMenu),
        )
    }

    pub(crate) fn view_inspect_modal<'a>(
        &self,
        inspect: &'a InspectState,
    ) -> Element<'a, Message> {
        let content = column![
            text("Commit Details").size(18),
            container(
                scrollable(text(&inspect.detail).size(12).font(Font::MONOSPACE)).height(400),
            )
            .padding(12)
            .width(Fill)
            .style(|_theme| container::Style {
                background: Some(color!(0x11111b).into()),
                border: iced::Border {
                    color: color!(0x45475a),
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            }),
            button("Close")
                .on_press(Message::DismissInspect)
                .padding([6, 20]),
        ]
        .spacing(12)
        .padding(24)
        .width(Fill);

        let modal_dialog = container(content)
            .style(|_theme| container::Style {
                background: Some(color!(0x1e1e2e).into()),
                border: iced::Border {
                    color: color!(0x585878),
                    width: 2.0,
                    radius: 8.0.into(),
                },
                shadow: iced::Shadow {
                    color: color!(0x000000),
                    offset: iced::Vector::new(0.0, 4.0),
                    blur_radius: 20.0,
                },
                ..Default::default()
            })
            .max_width(700);

        opaque(
            mouse_area(
                center(opaque(modal_dialog))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(|_theme| container::Style {
                        background: Some(
                            iced::Color {
                                a: 0.6,
                                ..color!(0x000000)
                            }
                            .into(),
                        ),
                        ..Default::default()
                    }),
            )
            .on_press(Message::DismissInspect),
        )
    }
}
