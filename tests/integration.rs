use daimon::{Config, DaimonError};

#[test]
fn error_display_agent_not_found() {
    let err = DaimonError::AgentNotFound("test-agent".into());
    assert!(err.to_string().contains("test-agent"));
    assert!(err.to_string().contains("agent not found"));
}

#[test]
fn error_display_invalid_parameter() {
    let err = DaimonError::InvalidParameter("bad port".into());
    assert!(err.to_string().contains("bad port"));
}

#[test]
fn error_display_agent_already_exists() {
    let err = DaimonError::AgentAlreadyExists("dup-agent".into());
    assert!(err.to_string().contains("dup-agent"));
}

#[test]
fn error_display_supervisor() {
    let err = DaimonError::SupervisorError("process crashed".into());
    assert!(err.to_string().contains("process crashed"));
}

#[test]
fn error_display_ipc() {
    let err = DaimonError::IpcError("socket closed".into());
    assert!(err.to_string().contains("socket closed"));
}

#[test]
fn error_display_scheduler() {
    let err = DaimonError::SchedulerError("queue full".into());
    assert!(err.to_string().contains("queue full"));
}

#[test]
fn error_display_federation() {
    let err = DaimonError::FederationError("peer unreachable".into());
    assert!(err.to_string().contains("peer unreachable"));
}

#[test]
fn error_display_api() {
    let err = DaimonError::ApiError("bad request".into());
    assert!(err.to_string().contains("bad request"));
}

#[test]
fn error_display_storage() {
    let err = DaimonError::StorageError("disk full".into());
    assert!(err.to_string().contains("disk full"));
}

#[test]
fn error_from_io() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing file");
    let err = DaimonError::from(io_err);
    assert!(err.to_string().contains("missing file"));
}

#[test]
fn config_default_values() {
    let cfg = Config::default();
    assert_eq!(cfg.listen_addr, "127.0.0.1");
    assert_eq!(cfg.port, 8090);
    assert_eq!(cfg.data_dir, "/var/lib/agnos");
    assert_eq!(cfg.max_agents, 1000);
}

#[test]
fn config_serde_roundtrip() {
    let cfg = Config::default();
    let json = serde_json::to_string(&cfg).unwrap();
    let deserialized: Config = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.port, cfg.port);
    assert_eq!(deserialized.listen_addr, cfg.listen_addr);
    assert_eq!(deserialized.data_dir, cfg.data_dir);
    assert_eq!(deserialized.max_agents, cfg.max_agents);
}

#[test]
fn config_custom_values() {
    let json = r#"{"listen_addr":"0.0.0.0","port":9090,"data_dir":"/tmp/daimon","max_agents":50}"#;
    let cfg: Config = serde_json::from_str(json).unwrap();
    assert_eq!(cfg.listen_addr, "0.0.0.0");
    assert_eq!(cfg.port, 9090);
    assert_eq!(cfg.data_dir, "/tmp/daimon");
    assert_eq!(cfg.max_agents, 50);
}

#[tokio::test]
async fn api_serve_returns_ok() {
    let cfg = Config::default();
    let result = daimon::api::serve(&cfg).await;
    assert!(result.is_ok());
}
