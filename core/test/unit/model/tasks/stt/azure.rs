use super::*;
use crate::model::tasks::stt::tests::spawn_one_shot_mock;

#[test]
fn azure_stt_requires_a_safe_resource_root_and_deployment_name() {
    for (url, name) in [
        (None, "dictation"),
        (Some("http://resource.openai.azure.com/"), "dictation"),
        (Some("https://api.openai.com/"), "dictation"),
        (
            Some("https://resource.openai.azure.com.example.test/"),
            "dictation",
        ),
        (
            Some("https://user:dummy@resource.openai.azure.com/"),
            "dictation",
        ),
        (
            Some("https://resource.openai.azure.com/openai/deployments/dep"),
            "dictation",
        ),
        (
            Some("https://resource.openai.azure.com/?api-version=bad"),
            "dictation",
        ),
        (Some("https://resource.openai.azure.com/"), "../other"),
    ] {
        assert!(resource_url(url, name).is_err(), "{url:?} {name:?}");
    }
}

#[tokio::test]
async fn azure_stt_uses_deployment_path_and_api_key_and_decodes_text() {
    const TEST_KEY_ENV: &str = "YUOS_TEST_AZURE_STT_WIRE_KEY";
    std::env::set_var(TEST_KEY_ENV, "synthetic-azure-key");
    let provider = AzureStt::from_config(&SttConfig {
        provider: "azure".into(),
        model: "Team.Dictation-01".into(),
        base_url: Some("https://resource.openai.azure.com/".into()),
        api_key_env: Some(TEST_KEY_ENV.into()),
        ..Default::default()
    })
    .unwrap();
    std::env::remove_var(TEST_KEY_ENV);
    assert!(provider.is_configured());

    let url = provider.endpoint(SttMode::Transcribe);
    assert_eq!(
        url.as_str(),
        "https://resource.openai.azure.com/openai/deployments/Team.Dictation-01/audio/transcriptions?api-version=2024-10-21"
    );
    assert_eq!(
        provider.endpoint(SttMode::Translate).path(),
        "/openai/deployments/Team.Dictation-01/audio/translations"
    );
    let form = request_form(
        SttRequest {
            audio: b"mock audio".to_vec(),
            filename: "clip.wav".into(),
            language: Some("en".into()),
            prompt: None,
            response_format: None,
            temperature: None,
            mode: SttMode::Transcribe,
        },
        None,
        "json",
    )
    .unwrap();
    let client = Client::builder().no_proxy().build().unwrap();
    let mut request = AzureStt::request(&client, url, form, provider.api_key.clone().unwrap())
        .build()
        .unwrap();
    // Substitute the origin only in this test. Production never relaxes TLS or DNS checks.
    let (base, captured) =
        spawn_one_shot_mock(r#"{"text":"hello"}"#.into(), "HTTP/1.1 200 OK").await;
    let mut target = Url::parse(&base).unwrap();
    target.set_path(request.url().path());
    target.set_query(request.url().query());
    *request.url_mut() = target;
    let response = client.execute(request).await.unwrap();
    let transcript = provider
        .decode(response, "json", SttMode::Transcribe)
        .await
        .unwrap();
    assert_eq!(transcript.text, "hello");
    assert_eq!(transcript.model, "Team.Dictation-01");
    let wire = String::from_utf8(captured.await.unwrap()).unwrap();
    assert!(wire.contains("api-key: synthetic-azure-key"));
    assert!(!wire.to_ascii_lowercase().contains("authorization:"));
    assert!(wire.contains("name=\"file\"; filename=\"clip.wav\""));
    assert!(wire.contains("name=\"language\""));
    assert!(!wire.contains("name=\"model\""));
}
