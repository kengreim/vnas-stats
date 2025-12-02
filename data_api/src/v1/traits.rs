use std::cmp;
use chrono::{DateTime, Utc};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TimeError {
    #[error("end time must be after start time")]
    IllegalEndTime,
}

pub trait Session {
    fn start_time(&self) -> DateTime<Utc>;
    fn end_time(&self) -> Option<DateTime<Utc>>;

    fn falls_within(&self, start_time: DateTime<Utc>, end_time: DateTime<Utc>, now: DateTime<Utc>) -> Result<bool, TimeError>
    {
        if end_time <= start_time {
            Err(TimeError::IllegalEndTime)
        } else {
            let self_end_time_sanitized = self.end_time().unwrap_or(now);
            Ok(self.start_time() < end_time && self_end_time_sanitized > start_time)
        }
    }

    fn duration_seconds_within(&self, start_time: DateTime<Utc>, end_time: DateTime<Utc>, now: DateTime<Utc>) -> Result<i64, TimeError>
    {
        if !self.falls_within(start_time, end_time, now)? {
            Ok(0)
        } else {
            let start_time_sanitized = cmp::max(start_time, self.start_time());
            let end_time_sanitized = cmp::min(end_time, self.end_time().unwrap_or(now));
            let duration = end_time_sanitized - start_time_sanitized;
            Ok(duration.num_seconds())
        }
    }
}