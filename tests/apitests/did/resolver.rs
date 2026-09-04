use boson::did::{ResolutionMetadata, ResolutionOptions, ResolutionResult, ResolutionStatus};

#[test]
fn resolution_options_default() {
    let options = ResolutionOptions::default();
    assert!(options.use_cache);
    assert_eq!(options.valid_ttl, 0);
}

#[test]
fn resolution_result_success() {
    let metadata = ResolutionMetadata {
        created: None,
        updated: None,
        resolved: std::time::SystemTime::now(),
        deactivated: false,
        version: 1,
    };
    let result = ResolutionResult::success("value", Some(metadata));
    assert_eq!(result.status, ResolutionStatus::Success);
    assert_eq!(result.result, Some("value"));
    assert!(result.succeeded());
    assert!(!result.failed());
    assert!(result.result_metadata.is_some());
}

#[test]
fn resolution_result_not_found() {
    let result = ResolutionResult::<()>::not_found();
    assert_eq!(result.status, ResolutionStatus::NotFound);
    assert!(result.result.is_none());
    assert!(result.failed());
}

#[test]
fn resolution_result_invalid() {
    let result = ResolutionResult::<()>::invalid();
    assert_eq!(result.status, ResolutionStatus::Invalid);
    assert!(result.result.is_none());
    assert!(result.failed());
}
