use iced::widget::canvas;
use iced::{color, Event, Point, Size, Theme};

use crate::graph::layout::GraphRow;
use crate::message::Message;
use crate::style::{lane_color, DOT_RADIUS, LANE_WIDTH, ROW_HEIGHT};

/// Full-graph Canvas program that renders all rows in one widget.
#[derive(Clone, Debug)]
pub(crate) struct FullGraph {
    pub(crate) rows: Vec<GraphRow>,
    pub(crate) selected_index: Option<usize>,
}

impl canvas::Program<Message> for FullGraph {
    type State = canvas::Cache;

    fn update(
        &self,
        state: &mut Self::State,
        event: &Event,
        bounds: iced::Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        match event {
            Event::Mouse(iced::mouse::Event::ButtonPressed(btn)) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    let row_idx = (pos.y / ROW_HEIGHT) as usize;
                    if row_idx < self.rows.len() {
                        let msg = match btn {
                            iced::mouse::Button::Left => Message::SelectCommit(row_idx),
                            iced::mouse::Button::Right => Message::ShowContextMenu(row_idx),
                            _ => return None,
                        };
                        state.clear();
                        return Some(canvas::Action::publish(msg));
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let geometry = state.draw(renderer, bounds.size(), |frame| {
            for (i, row) in self.rows.iter().enumerate() {
                let y_off = i as f32 * ROW_HEIGHT;
                let mid_y = y_off + ROW_HEIGHT / 2.0;

                let bg = if self.selected_index == Some(i) {
                    Some(color!(0x45475a))
                } else if i % 2 == 0 {
                    Some(color!(0x1e1e2e))
                } else {
                    Some(color!(0x181825))
                };
                if let Some(c) = bg {
                    frame.fill_rectangle(
                        Point::new(0.0, y_off),
                        Size::new(bounds.width, ROW_HEIGHT),
                        c,
                    );
                }

                for &(from_col, to_col, color_idx) in &row.incoming {
                    let from_x = from_col as f32 * LANE_WIDTH + LANE_WIDTH / 2.0;
                    let to_x = to_col as f32 * LANE_WIDTH + LANE_WIDTH / 2.0;
                    let path =
                        canvas::Path::line(Point::new(from_x, y_off), Point::new(to_x, mid_y));
                    frame.stroke(
                        &path,
                        canvas::Stroke::default()
                            .with_color(lane_color(color_idx))
                            .with_width(2.0),
                    );
                }

                for &(from_col, to_col, color_idx) in &row.outgoing {
                    let from_x = from_col as f32 * LANE_WIDTH + LANE_WIDTH / 2.0;
                    let to_x = to_col as f32 * LANE_WIDTH + LANE_WIDTH / 2.0;
                    let path = canvas::Path::line(
                        Point::new(from_x, mid_y),
                        Point::new(to_x, y_off + ROW_HEIGHT),
                    );
                    frame.stroke(
                        &path,
                        canvas::Stroke::default()
                            .with_color(lane_color(color_idx))
                            .with_width(2.0),
                    );
                }

                let dot_center =
                    Point::new(row.commit_col as f32 * LANE_WIDTH + LANE_WIDTH / 2.0, mid_y);
                let dot = canvas::Path::circle(dot_center, DOT_RADIUS);
                frame.fill(&dot, lane_color(row.commit_color));
            }
        });

        vec![geometry]
    }
}
