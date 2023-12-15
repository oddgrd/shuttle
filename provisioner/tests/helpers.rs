use std::process::Command;

pub const PG_CONTAINER_NAME: &str = "shuttle_provisioner_test_pg";
pub const MONGODB_CONTAINER_NAME: &str = "shuttle_provisioner_test_mongodb";

/// Execute commands in `mongosh` via `docker exec` against the provided `database_name`
/// or against the `admin` database by default
pub fn exec_mongosh(command: &str, database_name: Option<&str>) -> String {
    let output = Command::new("docker")
        .args([
            "exec",
            MONGODB_CONTAINER_NAME,
            "mongosh",
            "--quiet",
            "--username",
            "mongodb",
            "--password",
            "password",
            "--authenticationDatabase",
            "admin",
            database_name.unwrap_or("admin"),
            "--eval",
            command,
        ])
        .output()
        .unwrap()
        .stdout;

    String::from_utf8(output).unwrap().trim().to_string()
}
