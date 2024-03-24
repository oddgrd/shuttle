use std::{
    collections::{BTreeMap, HashMap},
    iter::FromIterator,
    net::{Ipv4Addr, SocketAddr},
    ops::DerefMut,
    str::FromStr,
    sync::Mutex,
};

use anyhow::Context;
use async_trait::async_trait;
use core::future::Future;
use shuttle_common::secrets::Secret;
use shuttle_proto::runtime::StopReason;
use shuttle_service::{Error, ResourceFactory, Service};
use tokio::sync::{
    broadcast::{self, Sender},
    oneshot,
};

use crate::args::args;
use crate::print_version;

// uses custom macro instead of clap to reduce dependency weight
args! {
    pub struct Args {
        // The port to open the gRPC control layer on.
        // The address to expose for the service is given in the StartRequest.
        "--port" => pub port: u16,
    }
}

pub async fn start(loader: impl Loader + Send + 'static, runner: impl Runner + Send + 'static) {
    // `--version` overrides any other arguments.
    if std::env::args().any(|arg| arg == "--version") {
        print_version();
        return;
    }

    // let args = match Args::parse() {
    //     Ok(args) => args,
    //     Err(e) => {
    //         eprintln!("Runtime received malformed or incorrect args, {e}");
    //         let help_str = "[HINT]: Run your Shuttle app with `cargo shuttle run`";
    //         let wrapper_str = "-".repeat(help_str.len());
    //         eprintln!("{wrapper_str}\n{help_str}\n{wrapper_str}");
    //         return;
    //     }
    // };

    println!("{} {} executable started", crate::NAME, crate::VERSION);

    // this is handled after arg parsing to not interfere with --version above
    #[cfg(feature = "setup-tracing")]
    {
        use colored::Colorize;
        use tracing_subscriber::prelude::*;

        colored::control::set_override(true); // always apply color

        tracing_subscriber::registry()
            .with(tracing_subscriber::fmt::layer().without_time())
            .with(
                // let user override RUST_LOG in local run if they want to
                tracing_subscriber::EnvFilter::try_from_default_env()
                    // otherwise use our default
                    .or_else(|_| tracing_subscriber::EnvFilter::try_new("info,shuttle=trace"))
                    .unwrap(),
            )
            .init();

        println!(
            "{}",
            "Shuttle's default tracing subscriber is initialized!".yellow(),
        );
        println!("To disable it and use your own, check the docs: https://docs.shuttle.rs/configuration/logs");
    }

    // TODO: initiate the runtime client, call load, then call start with the response.
    // We will call these consecutively on startup, rather than waiting for the runner to call them.
    // We will send the load request to the deployer sidecar, which will inject a secret and send
    // it to the control plane to be fulfilled.
    // TODO: change below to only have a server for HC, client for the rest.
    // // Address to reach the sidecar service.
    // let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 3000);

    let client = reqwest::Client::new();

    println!("calling runner server");
    let response = client
        .get("http://runner:3000")
        .send()
        .await
        .context("failed to call control plane")
        .unwrap();

    println!("runner call status code: {}", response.status());

    let response_body = response
        .text()
        .await
        .context("failed to read response body")
        .unwrap();

    println!("provisioning call body: {response_body}");

    let alpha = Alpha::new(loader, runner);

    let load = alpha
        .load(LoadRequest {
            secrets: Default::default(),
            project_name: "test-proj".to_string(),
            env: "deployment".to_string(),
        })
        .await
        .unwrap();

    println!("load response: {:?}", load);

    let start = alpha
        .start(StartRequest {
            ip: "http://0.0.0.0:3002".to_string(),
            resources: Default::default(),
        })
        .await
        .unwrap();

    println!("start response: {:?}", start);

    // TODO: subscribe stop to keep service alive? Or can we just start it directly, not as a BG task?

    // let mut server_builder = Server::builder()
    //     .http2_keepalive_interval(Some(Duration::from_secs(60)))
    //     .layer(ExtractPropagationLayer);

    // let router = {
    //     let alpha = Alpha::new(loader, runner);

    //     let svc = RuntimeServer::new(alpha);
    //     server_builder.add_service(svc)
    // };

    // match router.serve(addr).await {
    //     Ok(_) => {}
    //     Err(e) => panic!("Error while serving address {addr}: {e}"),
    // };
}

pub struct Alpha<L, R> {
    // Mutexes are for interior mutability
    stopped_tx: Sender<(StopReason, String)>,
    kill_tx: Mutex<Option<oneshot::Sender<String>>>,
    loader: Mutex<Option<L>>,
    runner: Mutex<Option<R>>,
}

pub struct LoadRequest {
    secrets: HashMap<String, String>,
    project_name: String,
    env: String,
}

#[derive(Debug)]
pub struct LoadResponse {
    pub success: bool,
    pub message: String,
    pub resources: Vec<Vec<u8>>,
}

pub struct StartRequest {
    pub ip: String,
    pub resources: Vec<Vec<u8>>,
}

#[derive(Default, Debug)]
pub struct StartResponse {
    pub success: bool,
    pub message: String,
}

impl<L, R, S> Alpha<L, R>
where
    L: Loader + Send + 'static,
    R: Runner<Service = S> + Send + 'static,
    S: Service + 'static,
{
    pub fn new(loader: L, runner: R) -> Self {
        let (stopped_tx, _stopped_rx) = broadcast::channel(10);

        Self {
            stopped_tx,
            kill_tx: Mutex::new(None),
            loader: Mutex::new(Some(loader)),
            runner: Mutex::new(Some(runner)),
        }
    }

    // TODO: handle errors?
    async fn load(&self, req: LoadRequest) -> Result<LoadResponse, Error> {
        // Sorts secrets by key
        let secrets =
            BTreeMap::from_iter(req.secrets.into_iter().map(|(k, v)| (k, Secret::new(v))));

        let factory = ResourceFactory::new(req.project_name, secrets, req.env.parse().unwrap());

        let loader = self.loader.lock().unwrap().deref_mut().take().unwrap();

        // TODO: run directly and panic main thread if it fails?
        // send to new thread to catch panics
        let resources = match tokio::spawn(loader.load(factory)).await {
            Ok(res) => match res {
                Ok(resources) => resources,
                Err(error) => {
                    println!("loading service failed: {error:#}");
                    return Ok(LoadResponse {
                        success: false,
                        message: error.to_string(),
                        resources: vec![],
                    });
                }
            },
            Err(error) => {
                if error.is_panic() {
                    let panic = error.into_panic();
                    let msg = match panic.downcast_ref::<String>() {
                        Some(msg) => msg.to_string(),
                        None => match panic.downcast_ref::<&str>() {
                            Some(msg) => msg.to_string(),
                            None => "<no panic message>".to_string(),
                        },
                    };
                    println!("loading service panicked: {msg}");
                    return Ok(LoadResponse {
                        success: false,
                        message: msg,
                        resources: vec![],
                    });
                } else {
                    println!("loading service crashed: {error:#}");
                    return Ok(LoadResponse {
                        success: false,
                        message: error.to_string(),
                        resources: vec![],
                    });
                }
            }
        };

        Ok(LoadResponse {
            success: true,
            message: String::new(),
            resources,
        })
    }

    async fn start(&self, request: StartRequest) -> Result<StartResponse, Error> {
        let StartRequest { ip, resources } = request;
        let service_address = SocketAddr::from_str(&ip).context("invalid socket address")?;

        let runner = self.runner.lock().unwrap().deref_mut().take().unwrap();

        let stopped_tx = self.stopped_tx.clone();

        // TODO: run directly and panic main thread if it fails?
        // send to new thread to catch panics
        let service = match tokio::spawn(runner.run(resources)).await {
            Ok(res) => match res {
                Ok(service) => service,
                Err(error) => {
                    println!("starting service failed: {error:#}");
                    let _ = stopped_tx
                        .send((StopReason::Crash, error.to_string()))
                        .map_err(|e| println!("{e}"));
                    return Ok(StartResponse {
                        success: false,
                        message: error.to_string(),
                    });
                }
            },
            Err(error) => {
                if error.is_panic() {
                    let panic = error.into_panic();
                    let msg = match panic.downcast_ref::<String>() {
                        Some(msg) => msg.to_string(),
                        None => match panic.downcast_ref::<&str>() {
                            Some(msg) => msg.to_string(),
                            None => "<no panic message>".to_string(),
                        },
                    };

                    println!("loading service panicked: {msg}");
                    let _ = stopped_tx
                        .send((StopReason::Crash, msg.to_string()))
                        .map_err(|e| println!("{e}"));
                    return Ok(StartResponse {
                        success: false,
                        message: msg,
                    });
                }
                println!("loading service crashed: {error:#}");
                let _ = stopped_tx
                    .send((StopReason::Crash, error.to_string()))
                    .map_err(|e| println!("{e}"));
                return Ok(StartResponse {
                    success: false,
                    message: error.to_string(),
                });
            }
        };
        println!("Starting on {service_address}");

        let (kill_tx, kill_rx) = tokio::sync::oneshot::channel();
        *self.kill_tx.lock().unwrap() = Some(kill_tx);

        let handle = tokio::runtime::Handle::current();

        // start service as a background task with a kill receiver
        tokio::spawn(async move {
            let mut background = handle.spawn(service.bind(service_address));

            tokio::select! {
                res = &mut background => {
                    match res {
                        Ok(_) => {
                            println!("service stopped all on its own");
                            let _ = stopped_tx
                                .send((StopReason::End, String::new()))
                                .map_err(|e| println!("{e}"));
                        },
                        Err(error) => {
                            if error.is_panic() {
                                let panic = error.into_panic();
                                let msg = match panic.downcast_ref::<String>() {
                                    Some(msg) => msg.to_string(),
                                    None => match panic.downcast_ref::<&str>() {
                                        Some(msg) => msg.to_string(),
                                        None => "<no panic message>".to_string(),
                                    },
                                };

                                println!("service panicked: {msg}");
                                let _ = stopped_tx
                                    .send((StopReason::Crash, msg))
                                    .map_err(|e| println!("{e}"));
                            } else {
                                println!("service crashed: {error}");
                                let _ = stopped_tx
                                    .send((StopReason::Crash, error.to_string()))
                                    .map_err(|e| println!("{e}"));
                            }
                        },
                    }
                },
                message = kill_rx => {
                    match message {
                        Ok(_) => {
                            let _ = stopped_tx
                                .send((StopReason::Request, String::new()))
                                .map_err(|e| println!("{e}"));
                        }
                        Err(_) => println!("the kill sender dropped")
                    };

                    println!("will now abort the service");
                    background.abort();
                    background.await.unwrap().expect("to stop service");
                }
            }
        });

        Ok(StartResponse {
            success: true,
            ..Default::default()
        })
    }
}

#[async_trait]
pub trait Loader {
    async fn load(self, factory: ResourceFactory) -> Result<Vec<Vec<u8>>, shuttle_service::Error>;
}

#[async_trait]
impl<F, O> Loader for F
where
    F: FnOnce(ResourceFactory) -> O + Send,
    O: Future<Output = Result<Vec<Vec<u8>>, shuttle_service::Error>> + Send,
{
    async fn load(self, factory: ResourceFactory) -> Result<Vec<Vec<u8>>, shuttle_service::Error> {
        (self)(factory).await
    }
}

#[async_trait]
pub trait Runner {
    type Service: Service;

    async fn run(self, resources: Vec<Vec<u8>>) -> Result<Self::Service, shuttle_service::Error>;
}

#[async_trait]
impl<F, O, S> Runner for F
where
    F: FnOnce(Vec<Vec<u8>>) -> O + Send,
    O: Future<Output = Result<S, shuttle_service::Error>> + Send,
    S: Service,
{
    type Service = S;

    async fn run(self, resources: Vec<Vec<u8>>) -> Result<Self::Service, shuttle_service::Error> {
        (self)(resources).await
    }
}
