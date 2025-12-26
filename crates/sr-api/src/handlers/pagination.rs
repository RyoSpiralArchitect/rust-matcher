use crate::error::ApiError;

const MAX_LIMIT: i64 = 200;
const MAX_OFFSET: i64 = 10_000;

pub fn validate_pagination(limit: i64, offset: i64) -> Result<(i64, i64), ApiError> {
    if !(1..=MAX_LIMIT).contains(&limit) {
        return Err(ApiError::BadRequest(format!(
            "limit must be between 1 and {MAX_LIMIT}"
        )));
    }

    if !(0..=MAX_OFFSET).contains(&offset) {
        return Err(ApiError::BadRequest(format!(
            "offset must be between 0 and {MAX_OFFSET}"
        )));
    }

    Ok((limit, offset))
}
