use warp::{
    reject::{self, Reject},
    Filter,
};

use crate::controllers::QuizController;
use crate::models::UserState;

#[derive(Debug)]
struct Unauthorized;

impl Reject for Unauthorized {}

pub fn with_quiz_controller(
    quiz_controller: QuizController,
) -> impl Filter<Extract = (QuizController,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || quiz_controller.clone())
}

pub fn user_state(
    quiz_controller: QuizController,
) -> impl Filter<Extract = (UserState,), Error = warp::Rejection> + Clone {
    warp::header::optional("Authorization")
        .and(with_quiz_controller(quiz_controller))
        .and_then(
            move |auth: Option<String>, quiz_controller: QuizController| async move {
                match auth {
                    None => Ok(quiz_controller.create_user()),
                    Some(auth) => {
                        let mut parts = auth.splitn(2, ' ');
                        let kind = parts.next().ok_or_else(|| reject::custom(Unauthorized))?;
                        let value = parts.next().ok_or_else(|| reject::custom(Unauthorized))?;

                        if !kind.eq_ignore_ascii_case("userstate") {
                            return Err(reject::custom(Unauthorized));
                        }

                        quiz_controller
                            .decode_user(value)
                            .map_err(|_err| reject::custom(Unauthorized))
                    }
                }
            },
        )
}
