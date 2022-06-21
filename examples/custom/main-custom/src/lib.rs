#[derive(Clone)]
pub struct MyService;

use shuttle_service::{Error, Service};

#[shuttle_service::async_trait]
impl Service for MyService {
    async fn bind(
        mut self: Box<Self>,
        _addr: std::net::SocketAddr,
    ) -> Result<(), shuttle_service::error::Error> {
        println!("service is binding");
        Ok(())
    }
}

async fn shuttle() -> Result<MyService, Error> {
    Ok(MyService {})
}

async fn __shuttle_wrapper(
    _factory: &mut dyn shuttle_service::Factory,
    runtime: &shuttle_service::Runtime,
    logger: Box<dyn shuttle_service::log::Log>,
) -> Result<Box<dyn Service>, Error> {
    runtime
        .spawn_blocking(move || {
            println!("setting logger");
            shuttle_service::log::set_boxed_logger(logger)
                .map(|()| {
                    shuttle_service::log::set_max_level(shuttle_service::log::LevelFilter::Info)
                })
                .expect("logger set should succeed");
        })
        .await
        .unwrap();

    runtime
        .spawn(async {
            println!("calling 'main'");
            shuttle().await.map(|ok| {
                let r: Box<dyn shuttle_service::Service> = Box::new(ok);
                r
            })
        })
        .await
        .unwrap()
}

fn __binder(// service: Box<dyn Service>,
    // runtime: &shuttle_service::Runtime,
) {
    println!("in __binder");
    // runtime.spawn_blocking(|| {
    //     println!("in blocking");
    // });
    // // .await
    // // .unwrap();

    // runtime.spawn(async move {
    //     println!("test 1.5");
    //     // tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    // });
    // .await
    // .unwrap();
}

#[no_mangle]
pub extern "C" fn _create_service() -> *mut shuttle_service::Bootstrapper {
    let builder: shuttle_service::StateBuilder<Box<dyn shuttle_service::Service>> =
        |factory, runtime, logger| Box::pin(__shuttle_wrapper(factory, runtime, logger));

    let bootstrapper = shuttle_service::Bootstrapper::new(builder, __binder);

    let boxed = Box::new(bootstrapper);
    Box::into_raw(boxed)
}
