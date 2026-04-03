use axum::{
    extract::Request,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "../../apps/web/dist"]
struct WebAssets;

pub async fn static_handler(req: Request) -> Response {
    let path = req.uri().path().trim_start_matches('/');

    if let Some(file) = WebAssets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, mime.as_ref())],
            file.data.to_vec(),
        )
            .into_response();
    }

    if path.starts_with("api/") || path == "api" {
        return (StatusCode::NOT_FOUND, "Not Found").into_response();
    }

    match WebAssets::get("index.html") {
        Some(index) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/html")],
            index.data.to_vec(),
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "Not Found").into_response(),
    }
}
