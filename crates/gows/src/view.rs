use iced::widget::{
    button, canvas, column, container, mouse_area, row, scrollable, stack, text, Column,
};
use iced::{color, Element, Fill, Font, Length};

use gitgobig_core::CommitEntry;

use crate::app::App;
use crate::graph::canvas::FullGraph;
use crate::message::Message;
use crate::style::{ResizeHandle, HANDLE_WIDTH, ROW_HEIGHT};

impl App {
    pub(crate) fn view(&self) -> Element<'_, Message> {
        let main_view = self.view_main();

        let mut layers: Vec<Element<'_, Message>> = vec![main_view];

        if self.branch_dropdown_open {
            layers.push(self.view_branch_dropdown());
        }

        if let Some(ref cm) = self.context_menu {
            layers.push(self.view_context_menu(cm));
        }

        if let Some(ref inspect) = self.inspect {
            layers.push(self.view_inspect_modal(inspect));
        }

        stack(layers).into()
    }

    fn view_main(&self) -> Element<'_, Message> {
        if let Some(ref e) = self.error {
            return container(text(format!("Error: {e}")).size(14))
                .padding(20)
                .into();
        }

        let graph_col_width = self.graph_col_width;

        let branch_toolbar = self.view_branch_toolbar();
        let search_bar = self.view_search_bar();

        // Header row
        let header = container(
            row![
                container(text(""))
                    .width(Length::Fixed(graph_col_width)),
                Self::resize_handle(ResizeHandle::Graph),
                text("Hash")
                    .size(12)
                    .font(Font::MONOSPACE)
                    .width(Length::Fixed(self.hash_width)),
                Self::resize_handle(ResizeHandle::Hash),
                text("Message").size(12).width(Fill),
                Self::resize_handle(ResizeHandle::Date),
                text("Date").size(12).width(Length::Fixed(self.date_width)),
                Self::resize_handle(ResizeHandle::Author),
                text("Author")
                    .size(12)
                    .width(Length::Fixed(self.author_width)),
            ]
            .padding([4, 8])
            .align_y(iced::Alignment::Center),
        )
        .height(Length::Fixed(ROW_HEIGHT))
        .style(|_theme| container::Style {
            background: Some(color!(0x313244).into()),
            border: iced::Border {
                color: color!(0x45475a),
                width: 0.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        });

        let graph_height = self.commits.len() as f32 * ROW_HEIGHT;

        let graph_canvas: Element<'_, Message> = canvas::Canvas::new(FullGraph {
            rows: self.graph_rows.clone(),
            selected_index: self.selected_index,
        })
        .width(Length::Fixed(graph_col_width))
        .height(Length::Fixed(graph_height))
        .into();

        let text_rows: Vec<Element<'_, Message>> = self
            .commits
            .iter()
            .enumerate()
            .map(|(i, c)| self.view_text_row(i, c))
            .collect();

        let mut text_col = Column::with_children(text_rows).spacing(0);

        if !self.all_loaded {
            if self.loading {
                text_col = text_col.push(
                    container(text("Loading…").size(13))
                        .padding(8)
                        .width(Fill)
                        .center_x(Fill),
                );
            } else {
                text_col = text_col.push(
                    container(
                        button("Load more commits")
                            .on_press(Message::LoadMore)
                            .padding([4, 12]),
                    )
                    .padding(8)
                    .width(Fill)
                    .center_x(Fill),
                );
            }
        }

        let content = row![graph_canvas, Self::col_spacer(), text_col.width(Fill)];

        column![
            branch_toolbar,
            search_bar,
            header,
            scrollable(content).height(Fill)
        ]
        .spacing(0)
        .width(Fill)
        .height(Fill)
        .into()
    }

    pub(crate) fn view_text_row<'a>(
        &'a self,
        index: usize,
        c: &'a CommitEntry,
    ) -> Element<'a, Message> {
        let is_selected = self.selected_index == Some(index);

        let refs_display: Element<'_, Message> = if c.refs.is_empty() {
            text("").into()
        } else {
            text(format!(" ({})", c.refs))
                .size(12)
                .color(color!(0xf9e2af))
                .into()
        };

        let msg_col: Element<'_, Message> = row![
            text(&c.subject)
                .size(12)
                .wrapping(iced::widget::text::Wrapping::None),
            refs_display,
        ]
        .into();

        let row_content = row![
            text(&c.short_hash)
                .size(12)
                .font(Font::MONOSPACE)
                .color(color!(0x89b4fa))
                .width(Length::Fixed(self.hash_width)),
            Self::col_spacer(),
            container(msg_col).width(Fill).clip(true),
            Self::col_spacer(),
            text(&c.date).size(11).width(Length::Fixed(self.date_width)),
            Self::col_spacer(),
            text(&c.author)
                .size(12)
                .color(color!(0xa6e3a1))
                .width(Length::Fixed(self.author_width)),
        ]
        .padding([0, 8]);

        let bg_color = if is_selected {
            color!(0x45475a)
        } else if index.is_multiple_of(2) {
            color!(0x1e1e2e)
        } else {
            color!(0x181825)
        };

        mouse_area(
            container(row_content)
                .height(Length::Fixed(ROW_HEIGHT))
                .width(Fill)
                .style(move |_theme| container::Style {
                    background: Some(bg_color.into()),
                    ..Default::default()
                }),
        )
        .on_press(Message::SelectCommit(index))
        .on_right_press(Message::ShowContextMenu(index))
        .into()
    }

    pub(crate) fn resize_handle(handle: ResizeHandle) -> Element<'static, Message> {
        mouse_area(
            container(
                container(text(""))
                    .width(Length::Fixed(2.0))
                    .height(Fill)
                    .style(|_theme| container::Style {
                        background: Some(color!(0x585878).into()),
                        ..Default::default()
                    }),
            )
            .width(Length::Fixed(HANDLE_WIDTH))
            .height(Fill)
            .center_x(HANDLE_WIDTH),
        )
        .on_press(Message::DragStart(handle))
        .into()
    }

    pub(crate) fn col_spacer<'a>() -> Element<'a, Message> {
        container(text(""))
            .width(Length::Fixed(HANDLE_WIDTH))
            .into()
    }
}
