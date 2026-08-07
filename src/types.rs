#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Conversation,
    Project,
}

#[derive(Debug)]
pub struct SearchResults {
    pub items: Vec<SearchResult>,
    pub has_more: bool,
    pub skipped_mismatched: usize,
}

#[derive(Debug)]
pub struct SearchResult {
    pub node_id: i64,
    pub data: String,
    pub metadata: Option<String>,
}

#[derive(Debug)]
pub struct SearchResultsDebug {
    pub items: Vec<SearchResultDebug>,
    pub has_more: bool,
    pub skipped_mismatched: usize,
}

#[derive(Debug)]
pub struct SearchResultDebug {
    pub node_id: i64,
    pub data: String,
    pub metadata: Option<String>,
    pub score: f32,
    pub segment_id: i64,
    pub conversation_id: String,
}

#[derive(Debug)]
pub struct NodeContent {
    pub data: String,
    pub metadata: Option<String>,
}

#[derive(Debug)]
pub struct ConversationInfo {
    pub uuid: String,
    pub created_at: i64,
    pub segment_count: usize,
    pub node_count: usize,
}

#[derive(Debug)]
pub struct ProjectStats {
    pub total_segments: usize,
    pub mismatched_segments: usize,
    pub total_nodes: usize,
}

/// A single node marked for removal.
#[derive(Debug, Clone)]
pub struct ForgetTarget {
    pub node_id: i64,
    pub segment_id: i64,
    pub conversation_id: String,
    pub data: String,
    pub metadata: Option<String>,
    /// Similarity for semantic candidates; `None` when targeted by node id.
    pub score: Option<f32>,
}

/// What a forget would do to one segment.
#[derive(Debug, Clone)]
pub struct SegmentImpact {
    pub segment_id: i64,
    pub node_count_before: i64,
    pub nodes_removed: i64,
    pub state: String,
    pub persist: bool,
    pub promoted: bool,
}

/// A reviewable, re-appliable set of removals.
///
/// Built by `plan_forget_nodes` / `plan_forget_matching` and executed by `apply_forget`,
/// so the preview and the applied action are the same decision — no re-search in between.
#[derive(Debug)]
pub struct ForgetPlan {
    pub targets: Vec<ForgetTarget>,
    pub impacts: Vec<SegmentImpact>,
    /// Segments where every remaining node is targeted; these get dropped entirely.
    pub emptied_segments: Vec<i64>,
    /// True when candidate search hit the limit — the list is not exhaustive.
    pub has_more: bool,
    /// Segments skipped because their embedding model no longer matches.
    pub skipped_mismatched: usize,
}

#[derive(Debug)]
pub struct ForgetOutcome {
    pub nodes_removed: usize,
    pub segments_rewritten: usize,
    pub segments_dropped: usize,
}
