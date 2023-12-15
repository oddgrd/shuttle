use portpicker::pick_unused_port;
use std::{
    process::Command,
    thread::sleep,
    time::{Duration, SystemTime},
};
use uuid::Uuid;

/// An instance of a docker container. It should be used by tests as a singleton
/// (e.g. once_cell::sync::Lazy), and any test logic that connects to it should
/// separate cases by creating a unique database. Also, the instance should
/// implement a destructor.
///
/// Example usage of a default Postgres instance:
///
/// ```
/// static PG: Lazy<DockerInstance> =
///     Lazy::new(|| DockerInstance::from_config(ContainerType::Postgres.into()));
///
/// // Ensure the instance is cleaned up when the test run ends.
/// #[dtor]
/// fn cleanup() {
///    PG.cleanup();
/// }
/// ```
/// For Postgres containers, we can import the [PostgresContainerExt] trait to get access to
/// utility methods, like one that creates a unique database.
///
/// ```
/// #[tokio::test]
/// async fn test_case() {
///     // Create a unique database and return the full URI.
///     let db_uri = PG.create_unique_database();
///     
///     // Test logic below, which can use `db_uri` to connect to the postgres instance.
/// }
///```
///
pub struct DockerInstance {
    pub container_name: String,
    pub uri: String,
    pub host_port: u16,
}

pub struct ContainerConfig<'a> {
    pub container_name: &'a str,
    pub db_engine: Option<&'a str>,
    pub env: &'a [&'a str],
    pub image: &'a str,
    pub image_tag: &'a str,
    pub is_ready_cmd: &'a [&'a str],
    pub port: u16,
}

pub enum ContainerType {
    Postgres,
    MongoDb,
    Stripe,
}

impl DockerInstance {
    pub fn from_config(
        ContainerConfig {
            container_name,
            db_engine,
            env,
            image,
            image_tag: image_version,
            is_ready_cmd,
            port,
        }: ContainerConfig<'_>,
    ) -> Self {
        let host_port = pick_unused_port().unwrap();
        let port_binding = format!("{}:{}", host_port, port);

        let mut args = vec![
            "run",
            "--rm",
            "--name",
            &container_name,
            "-p",
            &port_binding,
        ];

        args.extend(env.iter().flat_map(|e| ["-e", e]));

        let image = format!("{image}:{image_version}");
        args.push(&image);

        Command::new("docker").args(args).spawn().unwrap().stderr;

        println!("is ready command: {:?}", &is_ready_cmd);
        Self::wait_ready(Duration::from_secs(120), &is_ready_cmd);

        // DB containers start up twice. So wait for the first one to finish
        sleep(Duration::from_millis(350));
        Self::wait_ready(Duration::from_secs(120), &is_ready_cmd);

        // If db_engine is set, return a database URI.
        let uri = if let Some(db_engine) = db_engine {
            format!(
                "{}://{}:password@localhost:{}",
                db_engine, db_engine, host_port
            )
        } else {
            format!("localhost:{}", port)
        };

        Self {
            container_name: container_name.to_string(),
            uri,
            host_port,
        }
    }

    fn wait_ready(mut timeout: Duration, is_ready_cmd: &[&str]) {
        let mut now = SystemTime::now();
        while !timeout.is_zero() {
            let status = Command::new("docker")
                .args(is_ready_cmd)
                .output()
                .unwrap()
                .status;

            if status.success() {
                println!("{is_ready_cmd:?} succeeded...");
                return;
            }

            println!("{is_ready_cmd:?} did not succeed this time...");
            sleep(Duration::from_millis(350));

            timeout = timeout
                .checked_sub(now.elapsed().unwrap())
                .unwrap_or_default();
            now = SystemTime::now();
        }
        panic!("timed out while waiting for provisioner DB to come up");
    }

    pub fn cleanup(&self) {
        Command::new("docker")
            .args(["stop", &self.container_name])
            .output()
            .expect("failed to stop provisioner test DB container");
        Command::new("docker")
            .args(["rm", &self.container_name])
            .output()
            .expect("failed to remove provisioner test DB container");
    }
}

// Default configs for commonly used containers.
impl From<ContainerType> for ContainerConfig<'_> {
    fn from(container_type: ContainerType) -> Self {
        match container_type {
            ContainerType::Postgres => ContainerConfig {
                container_name: "postgres-test-container",
                image: "docker.io/library/postgres",
                image_tag: "14",
                db_engine: Some("postgres"),
                port: 5432,
                env: &["POSTGRES_PASSWORD=password", "PGUSER=postgres"],
                is_ready_cmd: &["exec", "postgres-test-container", "pg_isready"],
            },
            ContainerType::MongoDb => ContainerConfig {
                container_name: "mongodb-test-container",
                image: "docker.io/library/mongo",
                image_tag: "5.0.10",
                db_engine: Some("mongodb"),
                port: 27017,
                env: &[
                    "MONGO_INITDB_ROOT_USERNAME=mongodb",
                    "MONGO_INITDB_ROOT_PASSWORD=password",
                ],
                is_ready_cmd: &[
                    "exec",
                    "mongodb-test-container",
                    "mongosh",
                    "--quiet",
                    "--eval",
                    "db",
                ],
            },
            ContainerType::Stripe => ContainerConfig {
                container_name: "stripe-test-container",
                image_tag: "latest",
                image: "docker.io/adrienverge/localstripe",
                db_engine: None,
                port: 8420,
                env: Default::default(),
                is_ready_cmd: &[
                    "exec",
                    "stripe-test-container",
                    "curl",
                    "localhost:8420/v1/customers",
                    "-u",
                    "sk_test_123:",
                ],
            },
        }
    }
}

pub trait PostgresContainerExt {
    /// This endpoint should be used to get a unique connection string from
    /// the docker instance, so that the instance can be used by multiple
    /// clients in parallel, accessing different databases.
    fn create_unique_database(&self) -> String;
}

impl PostgresContainerExt for DockerInstance {
    fn create_unique_database(&self) -> String {
        // Get the PG uri first so the static PG is initialized.
        let db_name = Uuid::new_v4().to_string();
        exec_psql(
            &self.container_name,
            &format!(r#"CREATE DATABASE "{}";"#, db_name),
        );
        format!("{}/{}", &self.uri, db_name)
    }
}

/// Execute queries in `psql` via `docker exec`
pub fn exec_psql(container_name: &str, query: &str) -> String {
    let output = Command::new("docker")
        .args([
            "exec",
            container_name,
            "psql",
            "--username",
            "postgres",
            "--tuples-only",
            "--no-align",
            "--field-separator",
            ",",
            "--command",
            query,
        ])
        .output()
        .unwrap()
        .stdout;

    String::from_utf8(output).unwrap().trim().to_string()
}
