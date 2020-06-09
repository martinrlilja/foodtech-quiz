use anyhow::{anyhow, Result};
use chrono::Utc;
use csv;
use hex;
use rand::prelude::*;
use ring::hmac;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{File, OpenOptions},
    path::Path,
    sync::{Arc, Mutex},
};

use crate::models::{Code, Quiz, QuizQuestion, UserId, UserRecord, UserState, Wheel};

#[derive(Clone, Debug)]
pub struct QuizController {
    secret_key: Arc<hmac::Key>,
    codes: Arc<BTreeMap<String, Code>>,
    quiz: Arc<BTreeMap<String, Quiz>>,
    wheel: Arc<BTreeSet<String>>,
    user_writer: UserWriter,
}

impl QuizController {
    pub fn new<'a>(
        secret_key: hmac::Key,
        quiz: impl Iterator<Item = &'a Quiz>,
        codes: impl Iterator<Item = &'a Code>,
        wheel: impl Iterator<Item = &'a Wheel>,
        user_writer: UserWriter,
    ) -> QuizController {
        let quiz = quiz.map(|quiz| (quiz.name.clone(), quiz.clone())).collect();
        let codes = codes
            .map(|code| (code.code.clone(), code.clone()))
            .collect();
        let wheel = wheel.map(|wheel| wheel.name.clone()).collect();

        QuizController {
            secret_key: Arc::new(secret_key),
            quiz: Arc::new(quiz),
            codes: Arc::new(codes),
            wheel: Arc::new(wheel),
            user_writer,
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
            wheels: Default::default(),
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

    pub fn spin_wheel(&self, wheel: &str, user_state: &mut UserState) -> Option<u32> {
        const CHOICES: &[(u8, u8)] = &[(20, 3), (40, 2), (60, 1)];

        let wheel = self.wheel.get(wheel)?;
        match user_state.wheels.get(wheel) {
            None => {
                let mut rng = thread_rng();
                let (points, _weight) = CHOICES
                    .choose_weighted(&mut rng, |&(_points, weight)| weight)
                    .unwrap();
                user_state.wheels.insert(wheel.into(), *points);
                Some(*points as u32)
            }
            Some(_) => None,
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
            .sum::<u32>()
            + user_state
                .wheels
                .iter()
                .map(|(_wheel_name, points)| *points as u32)
                .sum::<u32>()
    }

    pub async fn register_user(
        &self,
        codes: &[impl AsRef<str>],
        email: &str,
        consent: bool,
        user_state: &UserState,
    ) -> u32 {
        let points = self.points(user_state);
        let now = Utc::now();

        let codes = codes.into_iter().map(|code| code.as_ref()).collect::<BTreeSet<_>>();
        let codes = codes
            .iter()
            .filter_map(|code| self.codes.get(&code.trim().to_lowercase()))
            .filter(|code| code.valid_from <= now && code.valid_to >= now)
            .collect::<Vec<_>>();

        let extra_points: u32 = codes.iter().map(|code| code.points).sum();
        let points = points + extra_points;

        if points > 0 {
            let mut code_names = codes
                .iter()
                .map(|code| code.code.clone())
                .collect::<Vec<_>>();
            code_names.sort();
            let code_names = code_names.join(" ");

            let record = UserRecord {
                id: hex::encode(&user_state.id.0),
                email: email.into(),
                points,
                codes: code_names,
                consent,
                time: now,
            };

            let user_writer = self.user_writer.clone();
            let blocking_task = tokio::task::spawn_blocking(move || {
                user_writer.write(record).unwrap();
            });
            blocking_task.await.unwrap();

            points
        } else {
            points
        }
    }
}

#[derive(Clone, Debug)]
pub struct UserWriter {
    writer: Arc<Mutex<csv::Writer<File>>>,
}

impl UserWriter {
    pub fn new(path: impl AsRef<Path>) -> Result<UserWriter> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;

        let writer = csv::WriterBuilder::new()
            .has_headers(false)
            .from_writer(file);

        let writer = Arc::new(Mutex::new(writer));

        Ok(UserWriter { writer })
    }

    pub fn write(&self, record: UserRecord) -> Result<()> {
        let mut writer = self
            .writer
            .lock()
            .map_err(|_err| anyhow!("couldn't lock writer"))?;
        writer.serialize(record)?;
        writer.flush()?;

        Ok(())
    }
}
