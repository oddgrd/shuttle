mod generated;

// useful re-exports if types are needed in other crates
pub use prost;
pub use prost_types;
pub use tonic;

#[cfg(feature = "provisioner")]
pub mod provisioner {
    use std::fmt::Display;

    use shuttle_common::{
        database::{self, AwsRdsEngine, SharedEngine},
        DatabaseReadyInfo,
    };

    pub use super::generated::provisioner::*;

    impl From<DatabaseResponse> for DatabaseReadyInfo {
        fn from(response: DatabaseResponse) -> Self {
            DatabaseReadyInfo::new(
                response.engine,
                response.username,
                response.password,
                response.database_name,
                response.port,
                response.address_private,
                response.address_public,
            )
        }
    }

    impl From<database::Type> for database_request::DbType {
        fn from(db_type: database::Type) -> Self {
            match db_type {
                database::Type::Shared(engine) => {
                    let engine = match engine {
                        SharedEngine::Postgres => shared::Engine::Postgres(String::new()),
                        SharedEngine::MongoDb => shared::Engine::Mongodb(String::new()),
                    };
                    database_request::DbType::Shared(Shared {
                        engine: Some(engine),
                    })
                }
                database::Type::AwsRds(engine) => {
                    let config = RdsConfig {};
                    let engine = match engine {
                        AwsRdsEngine::Postgres => aws_rds::Engine::Postgres(config),
                        AwsRdsEngine::MariaDB => aws_rds::Engine::Mariadb(config),
                        AwsRdsEngine::MySql => aws_rds::Engine::Mysql(config),
                    };
                    database_request::DbType::AwsRds(AwsRds {
                        engine: Some(engine),
                    })
                }
            }
        }
    }

    impl From<database_request::DbType> for Option<database::Type> {
        fn from(db_type: database_request::DbType) -> Self {
            match db_type {
                database_request::DbType::Shared(Shared {
                    engine: Some(engine),
                }) => match engine {
                    shared::Engine::Postgres(_) => {
                        Some(database::Type::Shared(SharedEngine::Postgres))
                    }
                    shared::Engine::Mongodb(_) => {
                        Some(database::Type::Shared(SharedEngine::MongoDb))
                    }
                },
                database_request::DbType::AwsRds(AwsRds {
                    engine: Some(engine),
                }) => match engine {
                    aws_rds::Engine::Postgres(_) => {
                        Some(database::Type::AwsRds(AwsRdsEngine::Postgres))
                    }
                    aws_rds::Engine::Mysql(_) => Some(database::Type::AwsRds(AwsRdsEngine::MySql)),
                    aws_rds::Engine::Mariadb(_) => {
                        Some(database::Type::AwsRds(AwsRdsEngine::MariaDB))
                    }
                },
                database_request::DbType::Shared(Shared { engine: None })
                | database_request::DbType::AwsRds(AwsRds { engine: None }) => None,
            }
        }
    }

    impl Display for aws_rds::Engine {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Mariadb(_) => write!(f, "mariadb"),
                Self::Mysql(_) => write!(f, "mysql"),
                Self::Postgres(_) => write!(f, "postgres"),
            }
        }
    }
}

#[cfg(feature = "runtime")]
pub mod runtime {
    pub use super::generated::runtime::*;
}

#[cfg(feature = "resource-recorder")]
pub mod resource_recorder {
    use anyhow::Context;
    use std::str::FromStr;

    pub use super::generated::resource_recorder::*;

    impl TryFrom<record_request::Resource> for shuttle_common::resource::Response {
        type Error = anyhow::Error;

        fn try_from(resource: record_request::Resource) -> Result<Self, Self::Error> {
            let r#type = shuttle_common::resource::Type::from_str(resource.r#type.as_str())
                .map_err(anyhow::Error::msg)
                .context("resource type should have a valid resource string")?;
            let response = shuttle_common::resource::Response {
                r#type,
                config: serde_json::from_slice(&resource.config)
                    .context(format!("{} resource config should be valid JSON", r#type))?,
                data: serde_json::from_slice(&resource.data)
                    .context(format!("{} resource data should be valid JSON", r#type))?,
            };

            Ok(response)
        }
    }

    impl TryFrom<Resource> for shuttle_common::resource::Response {
        type Error = anyhow::Error;

        fn try_from(resource: Resource) -> Result<Self, Self::Error> {
            let r#type = shuttle_common::resource::Type::from_str(resource.r#type.as_str())
                .map_err(anyhow::Error::msg)
                .context("resource type should have a valid resource string")?;

            let response = shuttle_common::resource::Response {
                r#type,
                config: serde_json::from_slice(&resource.config)
                    .context(format!("{} resource config should be valid JSON", r#type))?,
                data: serde_json::from_slice(&resource.data)
                    .context(format!("{} resource data should be valid JSON", r#type))?,
            };

            Ok(response)
        }
    }
}

#[cfg(feature = "builder")]
pub mod builder {
    pub use super::generated::builder::*;
}

#[cfg(feature = "logger")]
pub mod logger {
    use std::str::FromStr;
    use std::time::Duration;

    use chrono::{NaiveDateTime, TimeZone, Utc};
    use prost::bytes::Bytes;
    use tokio::{select, sync::mpsc, time::interval};
    use tonic::{
        async_trait,
        codegen::{Body, StdError},
        Request,
    };
    use tracing::error;

    use shuttle_common::{
        log::{Backend, LogItem as LogItemCommon, LogRecorder},
        DeploymentId,
    };

    pub use super::generated::logger::*;

    impl From<LogItemCommon> for LogItem {
        fn from(value: LogItemCommon) -> Self {
            Self {
                deployment_id: value.id.to_string(),
                log_line: Some(LogLine {
                    tx_timestamp: Some(prost_types::Timestamp {
                        seconds: value.timestamp.timestamp(),
                        nanos: value.timestamp.timestamp_subsec_nanos() as i32,
                    }),
                    service_name: format!("{:?}", value.internal_origin),
                    data: value.line.into_bytes(),
                }),
            }
        }
    }

    impl From<LogItem> for LogItemCommon {
        fn from(value: LogItem) -> Self {
            value
                .log_line
                .expect("log item to have log line")
                .to_log_item_with_id(value.deployment_id.parse().unwrap_or_default())
        }
    }

    impl LogLine {
        pub fn to_log_item_with_id(self, deployment_id: DeploymentId) -> LogItemCommon {
            let LogLine {
                service_name,
                tx_timestamp,
                data,
            } = self;
            let tx_timestamp = tx_timestamp.expect("log to have timestamp");

            LogItemCommon {
                id: deployment_id,
                internal_origin: Backend::from_str(&service_name)
                    .expect("backend name to be valid"),
                timestamp: Utc.from_utc_datetime(
                    &NaiveDateTime::from_timestamp_opt(
                        tx_timestamp.seconds,
                        tx_timestamp.nanos.try_into().unwrap_or_default(),
                    )
                    .unwrap_or_default(),
                ),
                line: String::from_utf8(data).expect("line to be utf-8"),
            }
        }
    }

    impl<I> LogRecorder for Batcher<I>
    where
        I: VecReceiver<Item = LogItem> + Clone + 'static,
    {
        fn record(&self, log: LogItemCommon) {
            self.send(log.into());
        }
    }

    /// Adapter to some client which expects to receive a vector of items
    #[async_trait]
    pub trait VecReceiver: Send {
        type Item;

        async fn receive(&mut self, items: Vec<Self::Item>);
    }

    #[async_trait]
    impl<T> VecReceiver for logger_client::LoggerClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody> + Send + Sync + Clone,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = Bytes> + Send + 'static,
        T::Future: Send,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        type Item = LogItem;

        async fn receive(&mut self, items: Vec<Self::Item>) {
            // A log vector should never be received without any items. We clone the first item
            // here so we can use IDs to generate a rate limiting logline, which we will send to
            // the logger as a warning to the user that they are being rate limited.
            let Some(item) = items.first().cloned() else {
                error!("received log vector without any items");

                return;
            };

            let mut store_logs_request = Request::new(StoreLogsRequest { logs: items });

            store_logs_request.metadata_mut().insert(
                "x-shuttle-deployment-id",
                item.deployment_id
                    .parse()
                    .expect("deployment id should be parsed to ascii metadatavalue"),
            );

            if let Err(error) = self.store_logs(store_logs_request).await {
                match error.code() {
                    tonic::Code::Unavailable => {
                        if error.metadata().get("x-ratelimit-limit").is_some() {
                            let LogItem {
                                deployment_id,
                                log_line,
                            } = item;

                            let LogLine { service_name, .. } = log_line.unwrap();

                            let timestamp = Utc::now();

                            let new_item = LogItem {
                                deployment_id,
                                log_line: Some(LogLine {
                                    tx_timestamp: Some(prost_types::Timestamp {
                                        seconds: timestamp.timestamp(),
                                        nanos: timestamp.timestamp_subsec_nanos() as i32,
                                    }),
                                    service_name: Backend::Runtime(service_name.clone())
                                        .to_string(),
                                    data: "your application is producing too many logs, log recording is being rate limited".into(),
                                }),
                            };

                            // Give the rate limiter time to refresh, the duration we need to sleep
                            // for here is determined by the refresh rate of the rate limiter in
                            // the logger server. It is currently set to refresh one slot per 500ms,
                            // so we could get away with a duration of 500ms here, but we give it
                            // some extra time in case this rate limiting was caused by a short
                            // burst of activity.
                            tokio::time::sleep(Duration::from_millis(1500)).await;

                            if let Err(error) = self
                                // NOTE: the request to send this rate limiting log to the logger will
                                // also expend a slot in the logger rate limiter.
                                .store_logs(Request::new(StoreLogsRequest {
                                    logs: vec![new_item],
                                }))
                                .await
                            {
                                error!(
                                    error = &error as &dyn std::error::Error,
                                    "failed to send rate limiting warning to logger service"
                                );
                            };
                        } else {
                            error!(
                                error = &error as &dyn std::error::Error,
                                "failed to send batch logs to logger"
                            );
                        }
                    }
                    _ => {
                        error!(
                            error = &error as &dyn std::error::Error,
                            "failed to send batch logs to logger"
                        );
                    }
                };
            }
        }
    }

    /// Wrapper to batch together items before forwarding them to some vector receiver
    #[derive(Clone)]
    pub struct Batcher<I: VecReceiver> {
        tx: mpsc::UnboundedSender<I::Item>,
    }

    impl<I: VecReceiver + 'static> Batcher<I>
    where
        I::Item: Send,
    {
        /// Create a new batcher around inner with the given batch capacity.
        /// Items will be send when the batch has reached capacity or at the set interval. Whichever comes first.
        pub fn new(inner: I, capacity: usize, interval: Duration) -> Self {
            let (tx, rx) = mpsc::unbounded_channel();

            tokio::spawn(Self::batch(inner, rx, capacity, interval));

            Self { tx }
        }

        /// Create a batcher around inner. It will send a batch of items to inner if a capacity of 256 is reached
        /// or if an interval of 1 second is reached.
        pub fn wrap(inner: I) -> Self {
            Self::new(inner, 256, Duration::from_secs(1))
        }

        /// Send a single item into this batcher
        pub fn send(&self, item: I::Item) {
            if self.tx.send(item).is_err() {
                unreachable!("the receiver will never drop");
            }
        }

        /// Background task to forward the items once the batch capacity has been reached
        async fn batch(
            mut inner: I,
            mut rx: mpsc::UnboundedReceiver<I::Item>,
            capacity: usize,
            interval_duration: Duration,
        ) {
            let mut interval = interval(interval_duration);

            // Without this, the default behaviour will burst any missed tickers until they are caught up.
            // This will cause a flood which we want to avoid.
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            // Get past the first tick
            interval.tick().await;

            let mut cache = Vec::with_capacity(capacity);

            loop {
                select! {
                    item = rx.recv() => {
                        if let Some(item) = item {
                            cache.push(item);

                            if cache.len() == capacity {
                                let old_cache = cache;
                                cache = Vec::with_capacity(capacity);

                                inner.receive(old_cache).await;
                            }
                        } else {
                            // Sender dropped
                            return;
                        }
                    },
                    _ = interval.tick() => {
                        if !cache.is_empty() {
                            let old_cache = cache;
                            cache = Vec::with_capacity(capacity);

                            inner.receive(old_cache).await;
                        }
                    }
                }
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use std::{
            sync::{Arc, Mutex},
            time::Duration,
        };

        use tokio::time::sleep;
        use tonic::async_trait;

        use super::{Batcher, VecReceiver};

        #[derive(Default, Clone)]
        struct MockGroupReceiver(Arc<Mutex<Option<Vec<u32>>>>);

        #[async_trait]
        impl VecReceiver for MockGroupReceiver {
            type Item = u32;

            async fn receive(&mut self, items: Vec<Self::Item>) {
                *self.0.lock().unwrap() = Some(items);
            }
        }

        #[tokio::test]
        async fn capacity_reached() {
            let mock = MockGroupReceiver::default();
            let batcher = Batcher::new(mock.clone(), 2, Duration::from_secs(120));

            batcher.send(1);
            sleep(Duration::from_millis(50)).await;
            assert_eq!(*mock.0.lock().unwrap(), None);

            batcher.send(2);
            sleep(Duration::from_millis(50)).await;
            assert_eq!(*mock.0.lock().unwrap(), Some(vec![1, 2]));

            batcher.send(3);
            sleep(Duration::from_millis(50)).await;
            assert_eq!(*mock.0.lock().unwrap(), Some(vec![1, 2]));

            batcher.send(4);
            sleep(Duration::from_millis(50)).await;
            assert_eq!(*mock.0.lock().unwrap(), Some(vec![3, 4]));
        }

        #[tokio::test]
        async fn interval_reached() {
            let mock = MockGroupReceiver::default();
            let batcher = Batcher::new(mock.clone(), 2, Duration::from_millis(300));

            sleep(Duration::from_millis(500)).await;
            assert_eq!(
                *mock.0.lock().unwrap(),
                None,
                "we should never send something when the cache is empty"
            );

            batcher.send(1);
            sleep(Duration::from_millis(50)).await;
            assert_eq!(*mock.0.lock().unwrap(), None);

            sleep(Duration::from_millis(500)).await;
            assert_eq!(*mock.0.lock().unwrap(), Some(vec![1]));
        }
    }
}
