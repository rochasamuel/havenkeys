mod support;

#[tokio::test]
async fn health_reports_ok() {
    let server = support::TestServer::start().await;
    let res = server.get("/v1/health").send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["status"], "ok");
    server.cleanup().await;
}
