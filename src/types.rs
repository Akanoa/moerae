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
