use anyhow::{Error, Result};
use rand::prelude::*;
use ring::{digest, hmac};
use serde::{Deserialize, Serialize};
use std::{env, net::SocketAddr};
use tokio::fs;
use warp::{
    http::{self, Response},
    reject,
    reply::{self, Reply},
    Filter,
};

use controllers::{QuizController, UserWriter};
use models::{Config, UserState};

mod controllers;
mod filters;
mod models;

#[derive(Clone, Debug, Deserialize, Serialize)]
struct QuizQuestionReply<'a> {
    question: &'a str,
    choices: Vec<&'a str>,
    token: &'a str,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct QuizAnswerRequest {
    answer: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct QuizAnswerReply<'a> {
    is_correct: bool,
    correct: Vec<&'a str>,
    token: &'a str,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct WheelSpinReply<'a> {
    points: u32,
    token: &'a str,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CheckoutRequest {
    codes: Vec<String>,
    email: String,
    consent: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CheckoutReply {
    points: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct StatsReply {
    total_points: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ErrorReply {
    error: ErrorCode,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
enum ErrorCode {
    NotFound,
}

#[tokio::main]
async fn main() -> Result<()> {
    let bind_addr = env::var("BIND").unwrap_or_else(|_err| "127.0.0.1:3030".into());
    let bind_addr: SocketAddr = bind_addr.parse()?;

    let cors_origin = env::var("CORS_ORIGIN").unwrap_or_else(|_err| "http://localhost:1313".into());

    let secret_key = env::var("SECRET_KEY")
        .map_err(|err| Error::new(err))
        .and_then(|env| {
            let mut secret_key = [0u8; digest::SHA256_OUTPUT_LEN];
            hex::decode_to_slice(env, &mut secret_key)?;
            Ok(secret_key)
        })
        .or_else(|_err| -> Result<_> {
            let mut secret_key = [0u8; digest::SHA256_OUTPUT_LEN];
            rand::rngs::OsRng.fill(&mut secret_key);

            println!("No secret key was specified, generated a new secret key.");
            println!("Rerun with SECRET_KEY={}", hex::encode(secret_key));

            Ok(secret_key)
        })?;

    let secret_key = hmac::Key::new(hmac::HMAC_SHA256, secret_key.as_ref());

    let config = fs::read_to_string("quiz.toml").await?;
    let config: Config = toml::de::from_str(&config)?;

    let user_writer = UserWriter::new("users.csv")?;

    let quiz_controller = QuizController::new(
        secret_key,
        config.quiz.iter(),
        config.code.iter(),
        config.wheel.iter(),
        user_writer,
    );

    let get_quiz = warp::path!("quiz" / String)
        .and(warp::get())
        .and(filters::user_state(quiz_controller.clone()))
        .and(filters::with_quiz_controller(quiz_controller.clone()))
        .map(
            |quiz_name: String, user_state: UserState, quiz_controller: QuizController| {
                let question = quiz_controller.next_question(&quiz_name, &user_state);

                match question {
                    None => reply::with_status(
                        reply::json(&ErrorReply {
                            error: ErrorCode::NotFound,
                        }),
                        http::StatusCode::NOT_FOUND,
                    )
                    .into_response(),
                    Some(question) => {
                        let mut choices = question
                            .correct
                            .iter()
                            .chain(question.incorrect.iter())
                            .map(|choice| choice.as_str())
                            .collect::<Vec<_>>();

                        let mut rng = thread_rng();
                        choices.shuffle(&mut rng);

                        let token = quiz_controller.encode_user(&user_state).unwrap();

                        let reply = QuizQuestionReply {
                            question: &question.question,
                            choices,
                            token: &token,
                        };

                        reply::json(&reply).into_response()
                    }
                }
            },
        );

    let post_quiz = warp::path!("quiz" / String)
        .and(warp::post())
        .and(warp::filters::body::json())
        .and(filters::user_state(quiz_controller.clone()))
        .and(filters::with_quiz_controller(quiz_controller.clone()))
        .map(
            |quiz_name: String,
             body: QuizAnswerRequest,
             mut user_state: UserState,
             quiz_controller: QuizController| {
                let answer =
                    quiz_controller.answer_question(&quiz_name, &mut user_state, &body.answer);

                match answer {
                    None => reply::with_status(
                        reply::json(&ErrorReply {
                            error: ErrorCode::NotFound,
                        }),
                        http::StatusCode::NOT_FOUND,
                    )
                    .into_response(),
                    Some((is_correct, question)) => {
                        let correct = question.correct.iter().map(|c| c.as_str()).collect();
                        let token = quiz_controller.encode_user(&user_state).unwrap();

                        let reply = QuizAnswerReply {
                            is_correct,
                            correct,
                            token: &token,
                        };

                        reply::json(&reply).into_response()
                    }
                }
            },
        );

    let post_wheel = warp::path!("wheel" / String)
        .and(warp::post())
        .and(filters::user_state(quiz_controller.clone()))
        .and(filters::with_quiz_controller(quiz_controller.clone()))
        .map(
            |wheel_name: String, mut user_state: UserState, quiz_controller: QuizController| {
                let wheel = quiz_controller.spin_wheel(&wheel_name, &mut user_state);

                match wheel {
                    None => reply::with_status(
                        reply::json(&ErrorReply {
                            error: ErrorCode::NotFound,
                        }),
                        http::StatusCode::NOT_FOUND,
                    )
                    .into_response(),
                    Some(points) => {
                        let token = quiz_controller.encode_user(&user_state).unwrap();

                        let reply = WheelSpinReply {
                            points,
                            token: &token,
                        };

                        reply::json(&reply).into_response()
                    }
                }
            },
        );

    let stats = warp::path!("stats")
        .and(warp::get())
        .and(filters::user_state(quiz_controller.clone()))
        .and(filters::with_quiz_controller(quiz_controller.clone()))
        .map(|user_state: UserState, quiz_controller: QuizController| {
            let points = quiz_controller.points(&user_state);

            let reply = StatsReply {
                total_points: points,
            };

            reply::json(&reply).into_response()
        });

    let checkout = warp::path!("checkout")
        .and(warp::post())
        .and(warp::filters::body::json())
        .and(filters::user_state(quiz_controller.clone()))
        .and(filters::with_quiz_controller(quiz_controller.clone()))
        .and_then(|body: CheckoutRequest, user_state: UserState, quiz_controller: QuizController| async move {
            if !body.email.contains('@') || body.email.len() < 3 {
                let reply = ErrorReply { error: ErrorCode::NotFound };

                Ok(reply::with_status(
                        reply::json(&reply),
                        http::StatusCode::NOT_FOUND,
                    ).into_response())
            } else {
                let points = quiz_controller.register_user(&body.codes, &body.email, body.consent, &user_state).await;

                let reply = CheckoutReply { points };

                Ok::<_, reject::Rejection>(reply::json(&reply).into_response())
            }
        });

    let script = warp::path!("static" / "script.js")
        .and(warp::get())
        .map(|| {
            const SCRIPT: &str = include_str!("script.js");
            Response::builder()
                .header("Content-Type", "application/javascript")
                .body(SCRIPT)
        })
        .with(warp::compression::gzip());

    let cors = warp::cors()
        .allow_origin(cors_origin.as_str())
        .allow_methods(vec!["GET", "POST"])
        .allow_headers(vec!["Authorization", "Content-Type"]);

    let server = get_quiz
        .or(post_quiz)
        .or(post_wheel)
        .or(stats)
        .or(checkout)
        .or(script)
        .with(cors);

    warp::serve(server).run(bind_addr).await;

    Ok(())
}
