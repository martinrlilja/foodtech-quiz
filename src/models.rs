use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    pub quiz: Vec<Quiz>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Quiz {
    pub name: String,
    pub points: u32,
    pub questions: Vec<QuizQuestion>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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
