use moerae::types::Scope;
use moerae::{Config, Moerae};

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

/// One tempdir for the whole process, with MOERAE_HOME set exactly once.
///
/// MOERAE_HOME is process-global, so setting and clearing it per test raced: a test
/// could observe it unset mid-window and fall through to the real ~/.moerae. Tests are
/// isolated by project id instead — each gets its own hashed project directory.
fn base_dir() -> &'static tempfile::TempDir {
    static BASE: OnceLock<tempfile::TempDir> = OnceLock::new();
    BASE.get_or_init(|| {
        let tmp = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("MOERAE_HOME", tmp.path()) };
        tmp
    })
}

fn test_moerae(config: Option<Config>) -> Moerae {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    let project_id = format!("test-project-{}", COUNTER.fetch_add(1, Ordering::SeqCst));
    open_project(&project_id, config)
}

/// Opens a project by name, so a test can reopen the same one to mimic a second process.
fn open_project(project_id: &str, config: Option<Config>) -> Moerae {
    base_dir();
    let model_path = moerae::embedding::model::default_model_path();

    let mut builder = Moerae::builder(project_id);
    builder = builder.model_path(&model_path);
    if let Some(c) = config {
        builder = builder.config(c);
    }

    builder.build().unwrap()
}

#[test]
#[ignore] // Requires model file
fn smoke_test_put_and_search() {
    let m = test_moerae(None);
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
    let m = test_moerae(None);
    let mut conv = m.create_conversation().unwrap();

    let err = conv.put("", None, false);
    assert!(err.is_err());

    let err = conv.put("   \n\t  ", None, false);
    assert!(err.is_err());
}

#[test]
#[ignore]
fn empty_query_rejected() {
    let m = test_moerae(None);
    let conv = m.create_conversation().unwrap();

    let err = conv.search("", None, None);
    assert!(err.is_err());
}

#[test]
#[ignore]
fn duplicate_put_is_idempotent() {
    let m = test_moerae(None);
    let mut conv = m.create_conversation().unwrap();

    conv.put("Hello, world!", None, false).unwrap();
    conv.put("Hello, world!", None, false).unwrap(); // duplicate

    let stats = m.segment_stats().unwrap();
    assert_eq!(stats.total_nodes, 1); // only 1 node stored
}

#[test]
#[ignore]
fn persist_routes_to_separate_segment() {
    let m = test_moerae(None);
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
    let m = test_moerae(None);

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
    let m = test_moerae(None);
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
    let m = test_moerae(None);

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
    let m = test_moerae(None);

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
    let m = test_moerae(None);

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
    let m = test_moerae(None);
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
    let m = test_moerae(None);
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
    let m = test_moerae(Some(config));
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

// --- forget ---

#[test]
#[ignore]
fn forget_by_node_id_removes_from_search() {
    let m = test_moerae(None);
    let uuid = {
        let mut conv = m.create_conversation().unwrap();
        conv.put("The API rate limit is 1000 req/min", None, false).unwrap();
        conv.put("Rust is a systems programming language", None, false).unwrap();
        conv.uuid.clone()
    };

    let conv = m.conversation(&uuid).unwrap();
    let results = conv.search_debug("rate limit", None, None).unwrap();
    let node_id = results.items[0].node_id;
    assert!(results.items[0].data.contains("1000"));
    drop(conv);

    let outcome = m.forget_nodes(&[node_id]).unwrap();
    assert_eq!(outcome.nodes_removed, 1);

    let conv = m.conversation(&uuid).unwrap();
    let after = conv.search("rate limit", None, None).unwrap();
    assert!(
        !after.items.iter().any(|i| i.node_id == node_id),
        "forgotten node still returned by search"
    );
    // The sibling survived — the whole point of rewriting rather than dropping.
    let rust = conv.search("Rust systems language", None, None).unwrap();
    assert!(rust.items.iter().any(|i| i.data.contains("Rust")));
}

#[test]
#[ignore]
fn forget_by_query_plans_candidates() {
    let m = test_moerae(None);
    let uuid = {
        let mut conv = m.create_conversation().unwrap();
        conv.put("The API rate limit is 1000 req/min", None, false).unwrap();
        conv.put("API rate limit: 1000 per key per minute", None, false).unwrap();
        conv.put("Postgres runs on port 5432", None, false).unwrap();
        conv.uuid.clone()
    };

    let plan = m
        .plan_forget_matching(Some(&uuid), "api rate limit", None, Some(50), 0.80)
        .unwrap();

    assert!(plan.targets.len() >= 2, "both rate-limit facts should match");
    assert!(
        plan.targets.iter().all(|t| t.score.unwrap() >= 0.80),
        "min_score not enforced"
    );
    assert!(
        !plan.targets.iter().any(|t| t.data.contains("Postgres")),
        "unrelated fact matched"
    );
}

#[test]
#[ignore]
fn forget_plan_does_not_boost_relevancy() {
    let m = test_moerae(None);
    let uuid = {
        let mut conv = m.create_conversation().unwrap();
        conv.put("The API rate limit is 1000 req/min", None, false).unwrap();
        conv.uuid.clone()
    };

    let relevancy = |m: &Moerae| -> f32 {
        m.db()
            .conn
            .query_row("SELECT MAX(relevancy_score) FROM segments", [], |r| r.get(0))
            .unwrap()
    };

    let before = relevancy(&m);
    m.plan_forget_matching(Some(&uuid), "rate limit", None, Some(50), 0.5)
        .unwrap();
    assert_eq!(before, relevancy(&m), "planning a forget boosted relevancy");

    // And prove the flag is actually wired, not merely off everywhere.
    let conv = m.conversation(&uuid).unwrap();
    conv.search_debug("rate limit", None, None).unwrap();
    drop(conv);
    assert!(
        relevancy(&m) > before,
        "normal search should still boost — the no-boost path proves nothing otherwise"
    );
}

#[test]
#[ignore]
fn forget_survives_reopen() {
    let m = test_moerae(None);
    let uuid = {
        let mut conv = m.create_conversation().unwrap();
        conv.put("The API rate limit is 1000 req/min", None, false).unwrap();
        conv.put("Rust is a systems programming language", None, false).unwrap();
        conv.uuid.clone()
    };

    let conv = m.conversation(&uuid).unwrap();
    let node_id = conv.search_debug("rate limit", None, None).unwrap().items[0].node_id;
    drop(conv);

    m.forget_nodes(&[node_id]).unwrap();

    // Reopening runs close_orphan_segments, which rebuilds when node_count disagrees
    // with the index. If the recount were skipped this would loop forever.
    let conv = m.conversation(&uuid).unwrap();
    let after = conv.search("rate limit", None, None).unwrap();
    assert!(!after.items.iter().any(|i| i.node_id == node_id));

    let (node_count, _): (i64, i64) = m
        .db()
        .conn
        .query_row(
            "SELECT SUM(node_count), COUNT(*) FROM segments",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(node_count, 1, "node_count drifted from reality");
}

#[test]
#[ignore]
fn forget_then_put_corrected_fact() {
    let m = test_moerae(None);
    let uuid = {
        let mut conv = m.create_conversation().unwrap();
        conv.put("The API rate limit is 1000 req/min", None, false).unwrap();
        conv.uuid.clone()
    };

    let plan = m
        .plan_forget_matching(Some(&uuid), "api rate limit", None, Some(50), 0.80)
        .unwrap();
    m.apply_forget(&plan).unwrap();

    let mut conv = m.conversation(&uuid).unwrap();
    conv.put("The API rate limit is 2000 req/min", None, false).unwrap();

    let results = conv.search("api rate limit", None, None).unwrap();
    assert!(results.items.iter().any(|i| i.data.contains("2000")));
    assert!(
        !results.items.iter().any(|i| i.data.contains("1000")),
        "superseded fact still present"
    );
}

#[test]
#[ignore]
fn apply_forget_rejects_active_open_segment() {
    let m = test_moerae(None);

    let mut conv = m.create_conversation().unwrap();
    conv.put("The API rate limit is 1000 req/min", None, false).unwrap();
    let uuid = conv.uuid.clone();
    let node_id = conv.search_debug("rate limit", None, None).unwrap().items[0].node_id;

    let plan = m.plan_forget_nodes(&[node_id]).unwrap();

    // The handle is still live and the segment is still open.
    assert!(matches!(
        m.apply_forget(&plan),
        Err(moerae::ForgetError::SegmentInUse { .. })
    ));

    drop(conv);
    m.apply_forget(&plan).unwrap();

    let conv = m.conversation(&uuid).unwrap();
    assert!(conv.search("rate limit", None, None).unwrap().items.is_empty());
}

#[test]
#[ignore]
fn forget_by_query_finds_segment_left_open_by_a_previous_process() {
    // Every `moerae put` is its own process: it leaves the segment state='open' with no
    // index file on disk, and the next open repairs it. Planning a forget must run that
    // repair too, or it searches a segment with no index and silently matches nothing.
    let project = "test-project-crossprocess";

    let uuid = {
        let m = open_project(project, None);
        let mut conv = m.create_conversation().unwrap();
        conv.put("The API rate limit is 1000 req/min", None, false).unwrap();
        conv.uuid.clone()
    };

    // A second "process" over the same project directory.
    let m = open_project(project, None);
    let plan = m
        .plan_forget_matching(Some(&uuid), "api rate limit", None, Some(50), 0.80)
        .expect("planning must find content written by a previous process");
    assert_eq!(plan.targets.len(), 1);

    m.apply_forget(&plan).unwrap();

    let m = open_project(project, None);
    let conv = m.conversation(&uuid).unwrap();
    assert!(conv.search("api rate limit", None, None).unwrap().items.is_empty());
}
