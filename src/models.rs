use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pub code: Vec<Code>,
    pub quiz: Vec<Quiz>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Code {
    pub code: String,
    pub points: u32,
    pub valid_from: DateTime<Utc>,
    pub valid_to: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Quiz {
    pub name: String,
    pub points: u32,
    pub questions: Vec<QuizQuestion>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct QuizQuestion {
    pub question: String,
    pub correct: Vec<String>,
    pub incorrect: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UserId(pub [u8; 16]);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UserState {
    pub id: UserId,
    pub answers: BTreeMap<String, Vec<bool>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct UserRecord {
    pub id: String,
    pub email: String,
    pub points: u32,
    pub codes: String,
}
