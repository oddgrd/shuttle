use actix_web::{get, App, Responder};

#[get("/")]
async fn hello_world() -> impl Responder {
    "Hello, World!"
}

#[shuttle_service::main]
async fn actix_web() -> shuttle_service::ShuttleActixWeb<impl Fn() + Clone> {
    let factory = || App::new().service(hello_world);
    Ok(shuttle_service::AppFactory { factory })
}
