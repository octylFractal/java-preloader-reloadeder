pub fn new_http_client() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(std::time::Duration::from_secs(5)))
        .timeout_recv_response(Some(std::time::Duration::from_secs(10)))
        .timeout_recv_body(Some(std::time::Duration::from_secs(30)))
        .timeout_send_request(Some(std::time::Duration::from_secs(10)))
        .timeout_send_body(Some(std::time::Duration::from_secs(30)))
        .user_agent(concat!(
            env!("CARGO_PKG_NAME"),
            "/",
            env!("CARGO_PKG_VERSION"),
            " (",
            env!("CARGO_PKG_REPOSITORY"),
            ")",
        ))
        .https_only(true)
        .build()
        .new_agent()
}
