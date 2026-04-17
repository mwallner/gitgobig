use iced::widget::{
    button, checkbox, column, container, mouse_area, opaque, row, scrollable, text, text_input,
    Column,
};
use iced::{color, Element, Fill, Length};

use crate::app::App;
use crate::message::Message;

impl App {
    pub(crate) fn view_branch_toolbar(&self) -> Element<'_, Message> {
        let label = if self.branches_loading {
            "Branches (loading…)".to_string()
        } else if self.selected_branches.len() == self.all_branches.len() {
            format!("Branches (all {})", self.all_branches.len())
        } else {
            format!(
                "Branches ({}/{})",
                self.selected_branches.len(),
                self.all_branches.len()
            )
        };

        let toggle_btn = button(text(label).size(12))
            .on_press(Message::ToggleBranchDropdown)
            .padding([4, 12]);

        container(
            row![toggle_btn]
                .spacing(8)
                .padding([6, 8])
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

    pub(crate) fn view_branch_dropdown(&self) -> Element<'_, Message> {
        let filter_input = text_input("Filter branches…", &self.branch_filter_text)
            .on_input(Message::BranchFilterText)
            .size(12)
            .padding(4)
            .width(Fill);

        let select_all_btn = button(text("All").size(11))
            .on_press(Message::SelectAllBranches)
            .padding([2, 8]);
        let deselect_all_btn = button(text("None").size(11))
            .on_press(Message::DeselectAllBranches)
            .padding([2, 8]);

        let filter_lower = self.branch_filter_text.to_lowercase();
        let branch_items: Vec<Element<'_, Message>> = self
            .all_branches
            .iter()
            .filter(|b| filter_lower.is_empty() || b.to_lowercase().contains(&filter_lower))
            .map(|b| {
                let is_checked = self.selected_branches.contains(b);
                let branch_name = b.clone();
                container(
                    checkbox(is_checked)
                        .label(b.as_str())
                        .on_toggle(move |_| Message::ToggleBranch(branch_name.clone()))
                        .size(14)
                        .text_size(12),
                )
                .padding([2, 4])
                .into()
            })
            .collect();

        let list =
            scrollable(Column::with_children(branch_items).spacing(0)).height(Length::Fixed(300.0));

        let dropdown_content = container(
            column![
                row![filter_input, select_all_btn, deselect_all_btn]
                    .spacing(4)
                    .align_y(iced::Alignment::Center),
                list,
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
        .max_width(400);

        opaque(
            mouse_area(
                container(column![
                    container(text("")).height(Length::Fixed(36.0)),
                    container(opaque(dropdown_content)).padding([0, 8]),
                ])
                .width(Fill)
                .height(Fill)
                .style(|_theme| container::Style {
                    background: Some(
                        iced::Color {
                            a: 0.15,
                            ..color!(0x000000)
                        }
                        .into(),
                    ),
                    ..Default::default()
                }),
            )
            .on_press(Message::DismissBranchDropdown),
        )
    }
}
