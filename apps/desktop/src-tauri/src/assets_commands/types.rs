#[derive(Debug, Clone, Deserialize)]
pub(crate) struct AssetResolveRequest {
    pub(crate) asset_ref: String,
    pub(crate) kind: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct AssetResolveResult {
    pub(crate) asset_ref: String,
    pub(crate) kind: Option<String>,
    pub(crate) url: Option<String>,
}
