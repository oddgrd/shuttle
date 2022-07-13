use actix_service::ServiceFactory;
use actix_web::{get, App, Error, Responder};

#[get("/")]
async fn hello_world() -> impl Responder {
    "Hello, World!"
}

#[shuttle_service::main]
fn actix_web<S>() -> ShuttleActixWeb<S>
where
    S: ServiceFactory<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse,
            InitError = (),
            Config = actix_web::dev::AppConfig,
        >
        + 'static
        + Send,
    S::Error: Into<Error>,
    S::Service: 'static,
{
    let app = App::new().service(hello_world);
    Ok(app)
}
