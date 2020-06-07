use anyhow::{anyhow, Result};
use rand::prelude::*;
use ring::hmac;
use std::{collections::BTreeMap, sync::Arc};
use tokio::sync::Mutex;

use crate::models::{Quiz, QuizQuestion, UserId, UserState};

#[derive(Clone, Debug)]
pub struct QuizController {
    secret_key: Arc<hmac::Key>,
    quiz: Arc<BTreeMap<String, Quiz>>,
    lock: Arc<Mutex<()>>,
}

impl QuizController {
    pub fn new<'a>(secret_key: hmac::Key, quiz: impl Iterator<Item = &'a Quiz>) -> QuizController {
        let quiz = quiz.map(|quiz| (quiz.name.clone(), quiz.clone())).collect();

        QuizController {
            secret_key: Arc::new(secret_key),
            quiz: Arc::new(quiz),
            lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn create_user(&self) -> UserState {
        let id = {
            let mut id = [0u8; 16];
            rand::rngs::OsRng.fill(&mut id);
            UserId(id)
        };

        UserState {
            id,
            answers: Default::default(),
        }
    }

    pub fn decode_user(&self, token: &str) -> Result<UserState> {
        let mut parts = token.splitn(2, ':');
        let user_state = parts
            .next()
            .ok_or_else(|| anyhow!("bad authorization token"))?;
        let user_state = base64::decode_config(user_state, base64::URL_SAFE_NO_PAD)?;

        let signature = parts
            .next()
            .ok_or_else(|| anyhow!("bad authorization token"))?;
        let signature = base64::decode_config(signature, base64::URL_SAFE_NO_PAD)?;

        hmac::verify(&self.secret_key, &user_state, &signature)
            .map_err(|_err| anyhow!("invalid signature"))?;

        let state = bincode::deserialize(&user_state)?;
        Ok(state)
    }

    pub fn encode_user(&self, user_state: &UserState) -> Result<String> {
        let user_state = bincode::serialize(&user_state)?;

        let signature = hmac::sign(&self.secret_key, &user_state);

        let token = format!(
            "{}:{}",
            base64::encode_config(user_state, base64::URL_SAFE_NO_PAD),
            base64::encode_config(signature, base64::URL_SAFE_NO_PAD),
        );

        Ok(token)
    }

    pub fn next_question<'a>(
        &'a self,
        quiz_name: &str,
        user_state: &UserState,
    ) -> Option<&'a QuizQuestion> {
        let quiz = self.quiz.get(quiz_name);
        let answers = user_state.answers.get(quiz_name);

        match quiz {
            None => None,
            Some(quiz) => {
                let index = answers.map(|answers| answers.len()).unwrap_or(0);
                let question = quiz.questions.get(index);
                question
            }
        }
    }

    pub fn answer_question<'a>(
        &'a self,
        quiz_name: &str,
        user_state: &mut UserState,
        answer: &str,
    ) -> Option<(bool, &'a QuizQuestion)> {
        let next_question = self.next_question(quiz_name, user_state);

        match next_question {
            None => None,
            Some(next_question) => {
                let answer_matches = next_question
                    .correct
                    .iter()
                    .any(|correct| correct == answer);

                match user_state.answers.get_mut(quiz_name) {
                    None => {
                        user_state
                            .answers
                            .insert(quiz_name.into(), vec![answer_matches]);
                    }
                    Some(answers) => answers.push(answer_matches),
                };

                Some((answer_matches, next_question))
            }
        }
    }

    pub fn points(&self, user_state: &UserState) -> u32 {
        user_state
            .answers
            .iter()
            .filter_map(|(quiz_name, answers)| self.quiz.get(quiz_name).map(|quiz| (quiz, answers)))
            .map(|(quiz, answers)| {
                let correct_answers = answers.iter().filter(|&&a| a).count();
                (quiz.points * correct_answers as u32) / quiz.questions.len() as u32
            })
            .sum()
    }
}
