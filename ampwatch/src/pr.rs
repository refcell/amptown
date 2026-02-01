use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct PullRequest {
    pub number: u32,
    pub title: String,
    pub state: String,
    pub author: Author,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "headRefName")]
    pub head_ref_name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Author {
    pub login: String,
}
