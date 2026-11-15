/// Parse through the canonical adapter while keeping the public legacy runtime API.
pub(crate) fn parse_json(json: &str) -> Result<Block, Vec<String>> {
    let document =
        cdl_compat::from_json(json, cdl_compat::ReadOptions::default()).map_err(|errors| {
            errors
                .into_iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
        })?;
    let diagnostics = cdl_model::validate(&document);
    if !diagnostics.is_empty() {
        return Err(diagnostics.into_iter().map(|e| e.to_string()).collect());
    }
    let graph = cdl_compat::to_value(&document).map_err(|errors| {
        errors
            .into_iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
    })?;
    serde_json::from_value(graph).map_err(|e| vec![e.to_string()])
}

pub use cdl_compat::legacy::*;
