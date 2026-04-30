mod app;
mod async_git;
mod message;
mod update;
mod view;

use crate::app::App;

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title(app_title)
        .theme(App::theme)
        .run()
}

fn app_title(_app: &App) -> String {
    String::from("gitgobig")
}
