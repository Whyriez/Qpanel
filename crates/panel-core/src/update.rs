//! Stable release identifiers shared by the update API and privileged agent.
use crate::CoreError;

pub fn release_version(tag: &str) -> Result<(u32, u32, u32), CoreError> {
    let invalid =
        || CoreError::Validation("Versi harus berupa tag stabil vMAJOR.MINOR.PATCH".into());
    let body = tag.strip_prefix('v').ok_or_else(invalid)?;
    if body.len() > 26 {
        return Err(invalid());
    }
    let mut parts = body.split('.');
    let mut part = || {
        let value = parts.next().ok_or_else(invalid)?;
        if value.is_empty()
            || value.len() > 8
            || !value.bytes().all(|b| b.is_ascii_digit())
            || (value.len() > 1 && value.starts_with('0'))
        {
            return Err(invalid());
        }
        value.parse().map_err(|_| invalid())
    };
    let result = (part()?, part()?, part()?);
    if parts.next().is_some() {
        return Err(invalid());
    }
    Ok(result)
}
pub fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
