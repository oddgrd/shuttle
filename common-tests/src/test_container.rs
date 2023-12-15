use portpicker::pick_unused_port;
use std::{
    process::Command,
    thread::sleep,
    time::{Duration, SystemTime},
};
use uuid::Uuid;

/// A builder for docker test instances. It produces a [TestDockerInstance], which should be used by
/// tests as a singleton (e.g.once_cell::sync::Lazy).
///
/// Example usage:
///
/// ```
/// static PG: Lazy<TestDockerInstance> = Lazy::new(|| DockerInstanceBuilder::new(
///         "test-container",
///         format!("docker.io/library/postgres:{db_version}"),
///         5432,
///     )
///     // If we set the db_engine field the instance will have a db uri.
///     .db_engine("postgres")
///     .env(&["POSTGRES_PASSWORD=password", "PGUSER=postgres"])
///     .is_ready_cmd(&["exec", "test-container", "pg_isready"])
///     .build()
/// )
///
/// #[dtor]
/// fn cleanup() {
///    PG.cleanup();
/// }
///
/// ```
///
/// If we import the [PostgresContainerExt] trait, we can also create a database with a unique
/// name.
///
/// ```
/// #[tokio::test]
/// async fn test_case() {
///     // Create a unique DB in the postgres test container, and return the URI.
///     let db_uri = PG.create_unique_database();
///     
///     // Test logic below, which can use `db_uri` to connect to the postgres instance.
/// }
///```
///
#[derive(Default)]
pub struct DockerInstanceBuilder<'a> {
    container_name: String,
    image: String,
    db_engine: Option<&'a str>,
    env: &'a [&'a str],
    is_ready_cmd: &'a [&'a str],
    port: u16,
    host_port: Option<u16>,
}

impl<'a> DockerInstanceBuilder<'a> {
    pub fn new(container_name: &str, image: impl ToString, port: u16) -> Self {
        Self {
            container_name: container_name.to_string(),
            image: image.to_string(),
            port,
            ..Default::default()
        }
    }

    pub fn db_engine(mut self, engine: &'a str) -> Self {
        self.db_engine = Some(engine);
        self
    }

    pub fn env(mut self, env: &'a [&'a str]) -> Self {
        self.env = env;
        self
    }

    pub fn is_ready_cmd(mut self, cmd: &'a [&'a str]) -> Self {
        self.is_ready_cmd = cmd;
        self
    }

    pub fn host_port(mut self, host_port: u16) -> Self {
        self.host_port = Some(host_port);
        self
    }

    pub fn build(self) -> TestDockerInstance {
        let container_name = self.container_name;
        // todo: if let engine, return pg uri
        let host_port = self
            .host_port
            .unwrap_or_else(|| pick_unused_port().unwrap());
        let port_binding = format!("{}:{}", host_port, self.port);

        let mut args = vec![
            "run",
            "--rm",
            "--name",
            container_name.as_str(),
            "-p",
            &port_binding,
        ];

        args.extend(self.env.iter().flat_map(|e| ["-e", e]));

        args.push(self.image.as_str());

        Command::new("docker").args(args).spawn().unwrap();

        wait_ready(Duration::from_secs(120), self.is_ready_cmd);

        // The container enters the ready state and then reboots, sleep a little and then
        // check if it's ready again afterwards.
        sleep(Duration::from_millis(350));
        wait_ready(Duration::from_secs(120), self.is_ready_cmd);

        let uri = if let Some(db_engine) = self.db_engine {
            format!(
                "{}://{}:password@localhost:{}",
                db_engine, db_engine, host_port
            )
        } else {
            format!("localhost:{}", self.port)
        };

        TestDockerInstance {
            container_name,
            uri,
        }
    }
}

pub struct TestDockerInstance {
    pub container_name: String,
    pub uri: String,
}

impl TestDockerInstance {
    // Remove the docker container.
    pub fn cleanup(&self) {
        Command::new("docker")
            .args(["stop", self.container_name.as_str()])
            .output()
            .expect("failed to stop provisioner test DB container");
        Command::new("docker")
            .args(["rm", self.container_name.as_str()])
            .output()
            .expect("failed to remove provisioner test DB container");
    }
}

pub trait PostgresContainerExt {
    /// This endpoint should be used to get a unique connection string from
    /// the docker instance, so that the instance can be used by multiple
    /// clients in parallel, accessing different databases.
    fn create_unique_database(&self) -> String;
}

impl PostgresContainerExt for TestDockerInstance {
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

/// A utility function to easily create a postgres container.
pub fn postgres_test_container(db_version: u16, name: &str) -> TestDockerInstance {
    DockerInstanceBuilder::new(
        name,
        format!("docker.io/library/postgres:{db_version}"),
        5432,
    )
    .db_engine("postgres")
    .env(&["POSTGRES_PASSWORD=password", "PGUSER=postgres"])
    .is_ready_cmd(&["exec", name, "pg_isready"])
    .build()
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
