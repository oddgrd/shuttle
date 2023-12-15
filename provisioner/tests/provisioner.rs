mod helpers;
use ctor::dtor;
use helpers::exec_mongosh;
use once_cell::sync::Lazy;
use serde_json::Value;
use shuttle_common_tests::test_container::{ContainerConfig, ContainerType, DockerInstance};
use shuttle_proto::provisioner::shared;
use shuttle_provisioner::MyProvisioner;

static PG: Lazy<DockerInstance> = Lazy::new(|| {
    let mut config: ContainerConfig = ContainerType::Postgres.into();

    // The shared postgres instance is version 14.
    config.image_tag = "14";

    DockerInstance::from_config(config)
});

static MONGODB: Lazy<DockerInstance> =
    Lazy::new(|| DockerInstance::from_config(ContainerType::MongoDb.into()));

#[dtor]
fn cleanup() {
    PG.cleanup();
    MONGODB.cleanup();
}

mod needs_docker {
    use shuttle_common_tests::test_container::exec_psql;

    use super::*;

    #[tokio::test]
    async fn shared_db_role_does_not_exist() {
        let provisioner = MyProvisioner::new(
            &PG.uri,
            &MONGODB.uri,
            "fqdn".to_string(),
            "pg".to_string(),
            "mongodb".to_string(),
        )
        .await
        .unwrap();

        assert_eq!(
            exec_psql(
                &PG.container_name,
                "SELECT rolname FROM pg_roles WHERE rolname = 'user-not_exist'",
            ),
            ""
        );

        provisioner
            .request_shared_db("not_exist", shared::Engine::Postgres(String::new()))
            .await
            .unwrap();

        assert_eq!(
            exec_psql(
                &PG.container_name,
                "SELECT rolname FROM pg_roles WHERE rolname = 'user-not_exist'",
            ),
            "user-not_exist"
        );
    }

    #[tokio::test]
    async fn shared_db_role_does_exist() {
        let provisioner = MyProvisioner::new(
            &PG.uri,
            &MONGODB.uri,
            "fqdn".to_string(),
            "pg".to_string(),
            "mongodb".to_string(),
        )
        .await
        .unwrap();

        exec_psql(
            &PG.container_name,
            "CREATE ROLE \"user-exist\" WITH LOGIN PASSWORD 'temp'",
        );
        let password = exec_psql(
            &PG.container_name,
            "SELECT passwd FROM pg_shadow WHERE usename = 'user-exist'",
        );

        provisioner
            .request_shared_db("exist", shared::Engine::Postgres(String::new()))
            .await
            .unwrap();

        // Make sure password got cycled
        assert_ne!(
            exec_psql(
                &PG.container_name,
                "SELECT passwd FROM pg_shadow WHERE usename = 'user-exist'",
            ),
            password
        );
    }

    #[tokio::test]
    #[should_panic(
        expected = "CreateRole(\"error returned from database: cannot insert multiple commands into a prepared statement\""
    )]
    async fn injection_safe() {
        let provisioner = MyProvisioner::new(
            &PG.uri,
            &MONGODB.uri,
            "fqdn".to_string(),
            "pg".to_string(),
            "mongodb".to_string(),
        )
        .await
        .unwrap();

        provisioner
            .request_shared_db(
                "new\"; CREATE ROLE \"injected",
                shared::Engine::Postgres(String::new()),
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn shared_db_missing() {
        let provisioner = MyProvisioner::new(
            &PG.uri,
            &MONGODB.uri,
            "fqdn".to_string(),
            "pg".to_string(),
            "mongodb".to_string(),
        )
        .await
        .unwrap();

        assert_eq!(
            exec_psql(
                &PG.container_name,
                "SELECT datname FROM pg_database WHERE datname = 'db-missing'",
            ),
            ""
        );

        provisioner
            .request_shared_db("missing", shared::Engine::Postgres(String::new()))
            .await
            .unwrap();

        assert_eq!(
            exec_psql(
                &PG.container_name,
                "SELECT datname FROM pg_database WHERE datname = 'db-missing'",
            ),
            "db-missing"
        );
    }

    #[tokio::test]
    async fn shared_db_filled() {
        let provisioner = MyProvisioner::new(
            &PG.uri,
            &MONGODB.uri,
            "fqdn".to_string(),
            "pg".to_string(),
            "mongodb".to_string(),
        )
        .await
        .unwrap();

        exec_psql(
            &PG.container_name,
            "CREATE ROLE \"user-filled\" WITH LOGIN PASSWORD 'temp'",
        );
        exec_psql(
            &PG.container_name,
            "CREATE DATABASE \"db-filled\" OWNER 'user-filled'",
        );
        assert_eq!(
            exec_psql(
                &PG.container_name,
                "SELECT datname FROM pg_database WHERE datname = 'db-filled'",
            ),
            "db-filled"
        );

        provisioner
            .request_shared_db("filled", shared::Engine::Postgres(String::new()))
            .await
            .unwrap();

        assert_eq!(
            exec_psql(
                &PG.container_name,
                "SELECT datname FROM pg_database WHERE datname = 'db-filled'",
            ),
            "db-filled"
        );
    }

    #[tokio::test]
    async fn shared_mongodb_role_does_not_exist() {
        let provisioner = MyProvisioner::new(
            &PG.uri,
            &MONGODB.uri,
            "fqdn".to_string(),
            "pg".to_string(),
            "mongodb".to_string(),
        )
        .await
        .unwrap();

        let user = exec_mongosh(
            &MONGODB.container_name,
            "db.getUser(\"user-not_exist\")",
            Some("mongodb-not_exist"),
        );
        assert_eq!(user, "null");

        provisioner
            .request_shared_db("not_exist", shared::Engine::Mongodb(String::new()))
            .await
            .unwrap();

        let user = exec_mongosh(
            &MONGODB.container_name,
            "db.getUser(\"user-not_exist\")",
            Some("mongodb-not_exist"),
        );
        assert!(user.contains("mongodb-not_exist.user-not_exist"));
    }

    #[tokio::test]
    async fn shared_mongodb_role_does_exist() {
        let provisioner = MyProvisioner::new(
            &PG.uri,
            &MONGODB.uri,
            "fqdn".to_string(),
            "pg".to_string(),
            "mongodb".to_string(),
        )
        .await
        .unwrap();

        exec_mongosh(
            &MONGODB.container_name,
            r#"db.createUser({ 
            user: "user-exist", 
            pwd: "secure_password", 
            roles: [
                { role: "readWrite", db: "mongodb-exist" }
            ]
        })"#,
            Some("mongodb-exist"),
        );

        let user: Value = serde_json::from_str(&exec_mongosh(
            &MONGODB.container_name,
            r#"EJSON.stringify(db.getUser("user-exist", 
            { showCredentials: true }
        ))"#,
            Some("mongodb-exist"),
        ))
        .unwrap();

        // Extract the user's stored password hash key from the `getUser` output
        let user_stored_key = &user["credentials"]["SCRAM-SHA-256"]["storedKey"];
        assert_eq!(user["_id"], "mongodb-exist.user-exist");

        provisioner
            .request_shared_db("exist", shared::Engine::Mongodb(String::new()))
            .await
            .unwrap();

        let user: Value = serde_json::from_str(&exec_mongosh(
            &MONGODB.container_name,
            r#"EJSON.stringify(db.getUser("user-exist", 
            { showCredentials: true }
        ))"#,
            Some("mongodb-exist"),
        ))
        .unwrap();

        // Make sure it's the same user
        assert_eq!(user["_id"], "mongodb-exist.user-exist");

        // Make sure password got cycled by comparing password hash keys
        let user_cycled_key = &user["credentials"]["SCRAM-SHA-256"]["storedKey"];
        assert_ne!(user_stored_key, user_cycled_key);
    }
}
