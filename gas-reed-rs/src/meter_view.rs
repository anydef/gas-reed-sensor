use askama::{FastWritable, Template};
use embassy_rp::pac::usb::regs::NakPoll;
use embassy_rp::pio::StatusSource;
use heapless::String;
use picoserve::response::{File, Response, StatusCode};

#[derive(Template)]
#[template(path = "hello.html")]
struct HelloTemplate<'a> {
    name: &'a str,
}

pub async fn hello_handler() -> impl picoserve::response::IntoResponse {
    let template = HelloTemplate {
        name: "Embedded Rust",
    };
    let mut rendered: String<512> = String::new();
    match template.render_into(&mut rendered) {
        Ok(_) => (StatusCode::OK, ("Content-Type", "text/html"), rendered),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            ("Content-Type", "text/html"),
            String::<512>::try_from("Internal Server Error").unwrap(),
        ),
    }
}
