use moerae::types::Scope;
use moerae::{Config, Moerae};

fn test_moerae(config: Option<Config>) -> (Moerae, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let model_path = moerae::embedding::model::default_model_path();

    let mut builder = Moerae::builder("test-project");
    builder = builder.model_path(&model_path);
    if let Some(c) = config {
        builder = builder.config(c);
    }

    // Override MOERAE_HOME so project dir goes into tempdir
    unsafe { std::env::set_var("MOERAE_HOME", tmp.path()) };
    let m = builder.build().unwrap();
    unsafe { std::env::remove_var("MOERAE_HOME") };

    (m, tmp)
}

#[test]
#[ignore] // Requires model file
fn smoke_test_put_and_search() {
    let (m, _tmp) = test_moerae(None);
    let mut conv = m.create_conversation().unwrap();

    conv.put("The capital of France is Paris", None, false).unwrap();
    conv.put("Rust is a systems programming language", None, false).unwrap();
    conv.put("The sun is a star", None, false).unwrap();

    let results = conv.search("What is the capital of France?", None, None).unwrap();
    assert!(!results.items.is_empty());
    assert!(results.items[0].data.contains("Paris"));
}

#[test]
#[ignore]
fn empty_data_rejected() {
    let (m, _tmp) = test_moerae(None);
    let mut conv = m.create_conversation().unwrap();

    let err = conv.put("", None, false);
    assert!(err.is_err());

    let err = conv.put("   \n\t  ", None, false);
    assert!(err.is_err());
}

#[test]
#[ignore]
fn empty_query_rejected() {
    let (m, _tmp) = test_moerae(None);
    let conv = m.create_conversation().unwrap();

    let err = conv.search("", None, None);
    assert!(err.is_err());
}

#[test]
#[ignore]
fn duplicate_put_is_idempotent() {
    let (m, _tmp) = test_moerae(None);
    let mut conv = m.create_conversation().unwrap();

    conv.put("Hello, world!", None, false).unwrap();
    conv.put("Hello, world!", None, false).unwrap(); // duplicate

    let stats = m.segment_stats().unwrap();
    assert_eq!(stats.total_nodes, 1); // only 1 node stored
}

#[test]
#[ignore]
fn persist_routes_to_separate_segment() {
    let (m, _tmp) = test_moerae(None);
    let mut conv = m.create_conversation().unwrap();

    conv.put("regular data", None, false).unwrap();
    conv.put("persistent config", None, true).unwrap();

    let stats = m.segment_stats().unwrap();
    assert_eq!(stats.total_segments, 2); // one regular, one persist
    assert_eq!(stats.total_nodes, 2);
}

#[test]
#[ignore]
fn conversation_isolation() {
    let (m, _tmp) = test_moerae(None);

    let mut conv1 = m.create_conversation().unwrap();
    conv1.put("Alpha project uses Python", None, false).unwrap();
    let uuid1 = conv1.uuid.clone();
    drop(conv1);

    let mut conv2 = m.create_conversation().unwrap();
    conv2.put("Beta project uses Rust", None, false).unwrap();

    // Conv2 should only find its own data at conversation scope
    let results = conv2.search("programming language", None, None).unwrap();
    assert!(!results.items.is_empty());
    assert!(results.items[0].data.contains("Rust"));

    // Should NOT find conv1's data at conversation scope
    for item in &results.items {
        assert!(!item.data.contains("Python"));
    }
}

#[test]
#[ignore]
fn get_returns_full_content() {
    let (m, _tmp) = test_moerae(None);
    let mut conv = m.create_conversation().unwrap();

    conv.put("specific test data for get", None, false).unwrap();

    let results = conv.search("specific test data", None, None).unwrap();
    assert!(!results.items.is_empty());

    let node_id = results.items[0].node_id;
    let content = m.get(node_id).unwrap();
    assert_eq!(content.data, "specific test data for get");
}

#[test]
#[ignore]
fn conversation_resume() {
    let (m, _tmp) = test_moerae(None);

    let mut conv = m.create_conversation().unwrap();
    conv.put("persisted memory", None, false).unwrap();
    let uuid = conv.uuid.clone();
    drop(conv);

    // Resume
    let conv2 = m.conversation(&uuid).unwrap();
    let results = conv2.search("memory", None, None).unwrap();
    assert!(!results.items.is_empty());
    assert!(results.items[0].data.contains("persisted memory"));
}

#[test]
#[ignore]
fn delete_conversation_removes_data() {
    let (m, _tmp) = test_moerae(None);

    let mut conv = m.create_conversation().unwrap();
    conv.put("data to delete", None, false).unwrap();
    let uuid = conv.uuid.clone();
    drop(conv);

    m.delete_conversation(&uuid).unwrap();

    let list = m.list_conversations().unwrap();
    assert!(list.iter().all(|c| c.uuid != uuid));
}

#[test]
#[ignore]
fn list_conversations_works() {
    let (m, _tmp) = test_moerae(None);

    let conv1 = m.create_conversation().unwrap();
    let conv2 = m.create_conversation().unwrap();
    let uuid1 = conv1.uuid.clone();
    let uuid2 = conv2.uuid.clone();
    drop(conv1);
    drop(conv2);

    let list = m.list_conversations().unwrap();
    assert_eq!(list.len(), 2);

    let uuids: Vec<&str> = list.iter().map(|c| c.uuid.as_str()).collect();
    assert!(uuids.contains(&uuid1.as_str()));
    assert!(uuids.contains(&uuid2.as_str()));
}

#[test]
#[ignore]
fn search_with_limit() {
    let (m, _tmp) = test_moerae(None);
    let mut conv = m.create_conversation().unwrap();

    for i in 0..5 {
        conv.put(&format!("data item number {i}"), None, false).unwrap();
    }

    let results = conv.search("data item", None, Some(2)).unwrap();
    assert_eq!(results.items.len(), 2);
    assert!(results.has_more);

    let results_all = conv.search("data item", None, Some(10)).unwrap();
    assert_eq!(results_all.items.len(), 5);
    assert!(!results_all.has_more);
}

#[test]
#[ignore]
fn search_debug_returns_scores() {
    let (m, _tmp) = test_moerae(None);
    let mut conv = m.create_conversation().unwrap();

    conv.put("The sky is blue", None, false).unwrap();

    let results = conv.search_debug("color of sky", None, None).unwrap();
    assert!(!results.items.is_empty());
    assert!(results.items[0].score > 0.0);
    assert!(results.items[0].score <= 1.0);
    assert!(results.items[0].segment_id > 0);
    assert!(!results.items[0].conversation_id.is_empty());
}

#[test]
#[ignore]
fn metadata_required_for_large_content() {
    let mut config = Config::default();
    config.max_indexable_tokens = 5; // very small for testing
    let (m, _tmp) = test_moerae(Some(config));
    let mut conv = m.create_conversation().unwrap();

    // This text will exceed 5 tokens
    let large = "This is a reasonably long sentence that should exceed the tiny token threshold we set for testing purposes";

    // Without metadata — should fail
    let err = conv.put(large, None, false);
    assert!(matches!(err, Err(moerae::PutError::MetadataRequired { .. })));

    // With metadata — should succeed
    conv.put(large, Some("long sentence test"), false).unwrap();

    // Search by metadata content
    let results = conv.search("long sentence", None, None).unwrap();
    assert!(!results.items.is_empty());
}
