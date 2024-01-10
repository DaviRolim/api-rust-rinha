use axum_extra::extract::WithRejection;
use serde_json::json;
use std::result::Result as StdResult;
use std::sync::Arc;
use thiserror::Error;

use axum::{
    extract::Path,
    extract::State,
    extract::{rejection::JsonRejection, Query},
    http::{
        header::{self, HeaderMap},
        HeaderValue, StatusCode,
    },
    response::{IntoResponse, Response as AxumResponse},
    Json,
};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use sqlx::FromRow;
use uuid::Uuid; // Import NaiveDate from chrono crate

use crate::AppState;

type Result<T> = StdResult<T, ApiError>;
#[derive(Deserialize)]
pub struct PessoaQuery {
    // search [t]erm
    t: String,
}
// We derive `thiserror::Error`
#[allow(non_camel_case_types)]
#[derive(Debug, Error)]
pub enum ApiError {
    // The `#[from]` attribute generates `From<JsonRejection> for ApiError`
    // implementation. See `thiserror` docs for more information
    #[error(transparent)]
    JsonExtractorRejection(#[from] JsonRejection),
    // Default Errors
    #[error("Bad Request")]
    BAD_REQUEST,
    #[error("Unprocessable Entity")]
    UNPROCESSABLE_ENTITY,
    #[error("Not Found")]
    NOT_FOUND,
}

pub enum ApiResponse {
    Ok(AxumResponse),
    Created(String, AxumResponse),
}

#[derive(Debug, Deserialize)]
pub struct CriarPessoaDTO {
    pub apelido: String,
    pub nome: String,
    pub nascimento: String,
    pub stack: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Serialize, FromRow)] // TODO consider using query_as to avoid the impl boilerplate, I'm not using because it breaks for creating user, I should have something different to create user
pub struct PessoaDTO {
    pub id: Uuid,
    pub apelido: String,
    pub nome: String,
    pub nascimento: String,
    pub stack: Option<Vec<String>>,
}

impl IntoResponse for ApiResponse {
    fn into_response(self) -> AxumResponse {
        match self {
            ApiResponse::Ok(response) => response,
            ApiResponse::Created(location, response) => {
                let mut headers = HeaderMap::new();
                headers.insert(header::LOCATION, HeaderValue::from_str(&location).unwrap());
                (StatusCode::CREATED, headers, response.into_body()).into_response()
            }
        }
    }
}

impl From<StatusCode> for ApiError {
    fn from(status: StatusCode) -> Self {
        match status {
            StatusCode::BAD_REQUEST => ApiError::BAD_REQUEST,
            StatusCode::UNPROCESSABLE_ENTITY => ApiError::UNPROCESSABLE_ENTITY,
            StatusCode::NOT_FOUND => ApiError::NOT_FOUND,
            _ => ApiError::BAD_REQUEST,
        }
    }
}
// We implement `IntoResponse` so ApiError can be used as a response
impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let payload = json!({
            "message": self.to_string(),
            "origin": "with_rejection"
        });
        let code = match self {
            ApiError::JsonExtractorRejection(x) => match x {
                JsonRejection::JsonDataError(err) => {
                    println!("{:?}", &err.body_text());
                    let error_not_null_apelido = err
                        .body_text()
                        .contains("type: apelido: invalid type: null");
                    let error_not_null_nome =
                        err.body_text().contains("type: nome: invalid type: null");
                    let error_stack_string = err
                        .body_text()
                        .contains("type: stack: invalid type: string");
                    if error_not_null_apelido || error_not_null_nome || error_stack_string {
                        StatusCode::UNPROCESSABLE_ENTITY
                    } else {
                        StatusCode::BAD_REQUEST
                    }
                }
                JsonRejection::JsonSyntaxError(err) => {
                    println!("{:?}", &err);
                    StatusCode::BAD_REQUEST
                }
                JsonRejection::MissingJsonContentType(_) => StatusCode::UNSUPPORTED_MEDIA_TYPE,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            },
            ApiError::BAD_REQUEST => StatusCode::BAD_REQUEST,
            ApiError::UNPROCESSABLE_ENTITY => StatusCode::UNPROCESSABLE_ENTITY,
            ApiError::NOT_FOUND => StatusCode::NOT_FOUND,
        };
        (code, Json(payload)).into_response()
    }
}

// Start Region: Handlers
pub async fn get_user(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<ApiResponse> {
    let uuid = Uuid::parse_str(&id).map_err(|_| StatusCode::NOT_FOUND)?;
    let row = sqlx::query_as::<_, PessoaDTO>("SELECT id, nickname as apelido, name as nome, TO_CHAR(birthday, 'YYYY-MM-DD') as nascimento, string_to_array(Stack, ' | ') as stack FROM PERSON WHERE ID = $1")
        .bind(uuid)
        .fetch_one(&state.db)
        .await
        .map_err(|err| {
            println!("{:?}", &err);
            StatusCode::NOT_FOUND
        })?;

    Ok(ApiResponse::Ok(Json(row).into_response()))
}

pub async fn create_user(
    State(state): State<Arc<AppState>>,
    WithRejection(Json(payload), _): WithRejection<Json<CriarPessoaDTO>, ApiError>,
) -> Result<ApiResponse> {
    _validate_payload(&payload)?;

    let stack = match payload.stack {
        Some(stack) => stack.join(" | "),
        None => "".to_string(),
    };

    let birthday = NaiveDate::parse_from_str(&payload.nascimento, "%Y-%m-%d")
        .map_err(|_| ApiError::BAD_REQUEST)?;

    let result = sqlx::query_as::<_, (Uuid,)>(
        "INSERT INTO PERSON (NICKNAME, NAME, BIRTHDAY, STACK) VALUES ($1, $2, $3, $4) RETURNING *",
    )
    .bind(payload.apelido)
    .bind(payload.nome)
    .bind(birthday)
    .bind(stack)
    .fetch_one(&state.db)
    .await
    // .map(|row| PessoaDTO::from(&row))
    .map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)?;

    Ok(ApiResponse::Created(
        format!("/pessoas/{}", result.0),
        Json(result).into_response(),
    ))
}

pub async fn get_pessoas_by_search_term(
    State(state): State<Arc<AppState>>,
    term: Query<PessoaQuery>,
) -> Result<ApiResponse> {
    let term = term.t.to_owned();
    // TODO string_to_array might be slower than handling this on the rust side
    let rows: Vec<PessoaDTO> = sqlx::query_as("SELECT id, nickname as apelido, name as nome, TO_CHAR(birthday, 'YYYY-MM-DD') as nascimento, string_to_array(Stack, ' | ') as stack FROM PERSON WHERE search ilike '%' || $1 || '%' limit 50")
            .bind(term)
            .fetch_all(&state.db)
            .await
            .map_err(|_| StatusCode::NOT_FOUND)?;
    // .iter_mut()
    // .map(|row| PessoaDTO::from(&row))
    // .collect();

    Ok(ApiResponse::Ok(Json(rows).into_response()))
}

fn _validate_payload(payload: &CriarPessoaDTO) -> std::result::Result<(), StatusCode> {
    if let Some(stack) = &payload.stack {
        for item in stack {
            if item.len() > 32 {
                println!("Failed validate 1 {:?}", &payload);
                return Err(StatusCode::BAD_REQUEST);
            }
        }
    } else if payload.apelido.len() > 32
        || payload.nome.len() > 100
        || NaiveDate::parse_from_str(&payload.nascimento, "%Y-%m-%d").is_err()
    {
        println!("Failed validate 2 {:?}", &payload);
        return Err(StatusCode::BAD_REQUEST);
    }

    Ok(())
}
// End Region: Handlers
