use orb8::error::{Orb8Error, Result};

#[test]
fn test_error_types() {
    let err = Orb8Error::PodNotFound {
        name: "test-pod".to_string(),
        namespace: "default".to_string(),
    };

    assert!(err.to_string().contains("test-pod"));
    assert!(err.to_string().contains("default"));
}

#[test]
fn test_version_const() {
    assert!(!orb8::VERSION.is_empty());
}
