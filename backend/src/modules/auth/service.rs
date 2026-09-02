//! 本地账号密码、密码哈希校验、JWT 签发和 refresh token 生命周期。

use argon2::{
    password_hash::{phc::PasswordHash, PasswordHasher, PasswordVerifier},
    Argon2,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use projecty_entity::{jwt_refresh_tokens, operation_logs, users};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{config::Config, domain::permissions::SystemRole};

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("invalid account or password")]
    InvalidCredentials,
    #[error("invalid or expired token")]
    InvalidToken,
    #[error("user is inactive")]
    InactiveUser,
    #[error("invalid system role in database")]
    InvalidSystemRole,
    #[error("password does not meet the required format")]
    InvalidPassword,
    #[error("invalid profile input: {0}")]
    InvalidInput(String),
    #[error("email is already in use")]
    EmailAlreadyUsed,
    #[error("database error: {0}")]
    Database(#[from] sea_orm::DbErr),
    #[error("token error: {0}")]
    Token(#[from] jsonwebtoken::errors::Error),
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub account: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProfileRequest {
    pub display_name: String,
    pub email: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AuthSession {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: &'static str,
    pub expires_in: u64,
}

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub id: Uuid,
    pub account: String,
    pub display_name: String,
    pub email: Option<String>,
    pub system_role: SystemRole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    pub sub: String,
    pub account: String,
    pub system_role: SystemRole,
    pub token_type: TokenType,
    pub iss: String,
    pub iat: usize,
    pub exp: usize,
    pub jti: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TokenType {
    Access,
    Refresh,
}

pub fn hash_password(password: &str) -> Result<String, AuthError> {
    if password.is_empty() {
        return Err(AuthError::InvalidPassword);
    }
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|hash| hash.to_string())
        .map_err(|_| AuthError::InvalidPassword)
}

pub fn verify_password(password: &str, password_hash: &str) -> bool {
    PasswordHash::new(password_hash)
        .map(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        })
        .unwrap_or(false)
}

pub async fn login(
    db: &DatabaseConnection,
    config: &Config,
    account: &str,
    password: &str,
) -> Result<AuthSession, AuthError> {
    let user = users::Entity::find()
        .filter(users::Column::Account.eq(account.trim()))
        .filter(users::Column::IsActive.eq(true))
        .filter(users::Column::DeletedAt.is_null())
        .one(db)
        .await?
        .filter(|user| verify_password(password, &user.password_hash))
        .ok_or(AuthError::InvalidCredentials)?;

    let role = parse_system_role(&user.system_role)?;
    let txn = db.begin().await?;
    let session = issue_session(&txn, config, &user, role, true).await?;
    txn.commit().await?;
    Ok(session)
}

pub async fn refresh(
    db: &DatabaseConnection,
    config: &Config,
    refresh_token: &str,
) -> Result<AuthSession, AuthError> {
    let claims = decode_claims(config, refresh_token, TokenType::Refresh)?;
    let user_id = parse_user_id(&claims)?;
    let now = Utc::now();
    let txn = db.begin().await?;
    let stored = jwt_refresh_tokens::Entity::find()
        .filter(jwt_refresh_tokens::Column::JtiHash.eq(hash_jti(&claims.jti)))
        .filter(jwt_refresh_tokens::Column::RevokedAt.is_null())
        .filter(jwt_refresh_tokens::Column::ExpiresAt.gt(now))
        .one(&txn)
        .await?
        .ok_or(AuthError::InvalidToken)?;

    if stored.user_id != user_id {
        return Err(AuthError::InvalidToken);
    }
    let user = users::Entity::find_by_id(user_id)
        .filter(users::Column::IsActive.eq(true))
        .filter(users::Column::DeletedAt.is_null())
        .one(&txn)
        .await?
        .ok_or(AuthError::InactiveUser)?;
    let role = parse_system_role(&user.system_role)?;

    let mut revoked: jwt_refresh_tokens::ActiveModel = stored.into();
    revoked.revoked_at = Set(Some(now));
    revoked.update(&txn).await?;

    let session = issue_session(&txn, config, &user, role, false).await?;
    txn.commit().await?;
    Ok(session)
}

pub async fn logout(
    db: &DatabaseConnection,
    config: &Config,
    refresh_token: &str,
) -> Result<(), AuthError> {
    let claims = decode_claims(config, refresh_token, TokenType::Refresh)?;
    let mut token = jwt_refresh_tokens::Entity::find()
        .filter(jwt_refresh_tokens::Column::JtiHash.eq(hash_jti(&claims.jti)))
        .filter(jwt_refresh_tokens::Column::RevokedAt.is_null())
        .one(db)
        .await?
        .ok_or(AuthError::InvalidToken)?;
    token.revoked_at = Some(Utc::now());
    let model: jwt_refresh_tokens::ActiveModel = token.into();
    model.update(db).await?;
    Ok(())
}

pub async fn me(db: &DatabaseConnection, user_id: Uuid) -> Result<MeResponse, AuthError> {
    let user = users::Entity::find_by_id(user_id)
        .filter(users::Column::IsActive.eq(true))
        .filter(users::Column::DeletedAt.is_null())
        .one(db)
        .await?
        .ok_or(AuthError::InactiveUser)?;
    me_from_model(user)
}

/// 本人维护的基础资料:姓名与邮箱。账号、角色等仍由管理员在用户管理中调整。
pub async fn update_profile(
    db: &DatabaseConnection,
    user_id: Uuid,
    request: UpdateProfileRequest,
) -> Result<MeResponse, AuthError> {
    let user = users::Entity::find_by_id(user_id)
        .filter(users::Column::IsActive.eq(true))
        .filter(users::Column::DeletedAt.is_null())
        .one(db)
        .await?
        .ok_or(AuthError::InactiveUser)?;
    let display_name = normalize_display_name(&request.display_name)?;
    let email = normalize_email(request.email.as_deref())?;
    if user.display_name == display_name && user.email == email {
        return me_from_model(user);
    }
    let now = Utc::now();
    let mut active: users::ActiveModel = user.into();
    active.display_name = Set(display_name.clone());
    active.email = Set(email.clone());
    active.updated_at = Set(now);
    let updated = active.update(db).await.map_err(map_unique_email)?;
    operation_logs::ActiveModel {
        id: Set(Uuid::now_v7()),
        actor_user_id: Set(user_id),
        module: Set("users".to_owned()),
        action: Set("self_update".to_owned()),
        project_id: Set(None),
        task_id: Set(None),
        target_type: Set("user".to_owned()),
        target_id: Set(Some(user_id)),
        summary: Set("更新个人资料".to_owned()),
        diff: Set(Some(serde_json::json!({
            "display_name": display_name,
            "email": email,
        }))),
        snapshot: Set(None),
        created_at: Set(now),
    }
    .insert(db)
    .await?;
    me_from_model(updated)
}

fn me_from_model(user: users::Model) -> Result<MeResponse, AuthError> {
    Ok(MeResponse {
        id: user.id,
        account: user.account,
        display_name: user.display_name,
        email: user.email,
        system_role: parse_system_role(&user.system_role)?,
    })
}

fn normalize_display_name(raw: &str) -> Result<String, AuthError> {
    let display_name = raw.trim().to_owned();
    if display_name.is_empty() || display_name.chars().count() > 80 {
        return Err(AuthError::InvalidInput(
            "姓名不能为空且不超过 80 个字符".to_owned(),
        ));
    }
    Ok(display_name)
}

/// 邮箱可选:传 None 或空白视为清除;填写时做格式校验并要求全局唯一。
fn normalize_email(raw: Option<&str>) -> Result<Option<String>, AuthError> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let email = raw.trim().to_owned();
    if email.is_empty() {
        return Ok(None);
    }
    let valid = email.chars().count() <= 254
        && !email.chars().any(char::is_whitespace)
        && email.matches('@').count() == 1
        && {
            let (local, domain) = email.split_once('@').unwrap_or(("", ""));
            !local.is_empty()
                && local.chars().count() <= 64
                && domain.matches('.').count() >= 1
                && !domain.starts_with('.')
                && !domain.ends_with('.')
                && !domain.contains("..")
        };
    if !valid {
        return Err(AuthError::InvalidInput("邮箱格式不正确".to_owned()));
    }
    Ok(Some(email))
}

fn map_unique_email(error: sea_orm::DbErr) -> AuthError {
    let text = error.to_string().to_ascii_lowercase();
    if text.contains("email") || text.contains("users_email_key") {
        AuthError::EmailAlreadyUsed
    } else {
        AuthError::Database(error)
    }
}

pub async fn change_password(
    db: &DatabaseConnection,
    user_id: Uuid,
    current_password: &str,
    new_password: &str,
) -> Result<(), AuthError> {
    let user = users::Entity::find_by_id(user_id)
        .filter(users::Column::IsActive.eq(true))
        .filter(users::Column::DeletedAt.is_null())
        .one(db)
        .await?
        .ok_or(AuthError::InactiveUser)?;
    if !verify_password(current_password, &user.password_hash) {
        return Err(AuthError::InvalidCredentials);
    }
    let password_hash = hash_password(new_password)?;
    let now = Utc::now();
    let txn = db.begin().await?;
    let mut active: users::ActiveModel = user.into();
    active.password_hash = Set(password_hash);
    active.updated_at = Set(now);
    active.update(&txn).await?;

    let tokens = jwt_refresh_tokens::Entity::find()
        .filter(jwt_refresh_tokens::Column::UserId.eq(user_id))
        .filter(jwt_refresh_tokens::Column::RevokedAt.is_null())
        .all(&txn)
        .await?;
    for token in tokens {
        let mut active: jwt_refresh_tokens::ActiveModel = token.into();
        active.revoked_at = Set(Some(now));
        active.update(&txn).await?;
    }
    txn.commit().await?;
    Ok(())
}

pub fn decode_access_claims(config: &Config, token: &str) -> Result<JwtClaims, AuthError> {
    decode_claims(config, token, TokenType::Access)
}

fn decode_claims(
    config: &Config,
    token: &str,
    expected_type: TokenType,
) -> Result<JwtClaims, AuthError> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_issuer(std::slice::from_ref(&config.jwt_issuer));
    let claims = decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(config.jwt_secret.as_bytes()),
        &validation,
    )?
    .claims;
    if claims.token_type != expected_type {
        return Err(AuthError::InvalidToken);
    }
    Ok(claims)
}

async fn issue_session<C: ConnectionTrait + Send + Sync>(
    conn: &C,
    config: &Config,
    user: &users::Model,
    role: SystemRole,
    update_login: bool,
) -> Result<AuthSession, AuthError> {
    let now = Utc::now();
    let access_jti = Uuid::now_v7().to_string();
    let refresh_jti = Uuid::now_v7().to_string();
    let access_exp = now + Duration::seconds(config.access_token_ttl_seconds as i64);
    let refresh_exp = now + Duration::seconds(config.refresh_token_ttl_seconds as i64);
    let access_token = encode_claims(
        config,
        &user.id,
        &user.account,
        role,
        TokenType::Access,
        &access_jti,
        access_exp,
    )?;
    let refresh_token = encode_claims(
        config,
        &user.id,
        &user.account,
        role,
        TokenType::Refresh,
        &refresh_jti,
        refresh_exp,
    )?;

    jwt_refresh_tokens::ActiveModel {
        id: Set(Uuid::now_v7()),
        user_id: Set(user.id),
        jti_hash: Set(hash_jti(&refresh_jti)),
        issued_at: Set(now),
        expires_at: Set(refresh_exp),
        revoked_at: Set(None),
    }
    .insert(conn)
    .await?;

    if update_login {
        let mut active: users::ActiveModel = user.clone().into();
        active.last_login_at = Set(Some(now));
        active.updated_at = Set(now);
        active.update(conn).await?;
    }

    Ok(AuthSession {
        access_token,
        refresh_token,
        token_type: "Bearer",
        expires_in: config.access_token_ttl_seconds,
    })
}

fn encode_claims(
    config: &Config,
    user_id: &Uuid,
    account: &str,
    role: SystemRole,
    token_type: TokenType,
    jti: &str,
    expires_at: chrono::DateTime<Utc>,
) -> Result<String, AuthError> {
    let claims = JwtClaims {
        sub: user_id.to_string(),
        account: account.to_owned(),
        system_role: role,
        token_type,
        iss: config.jwt_issuer.clone(),
        iat: Utc::now().timestamp() as usize,
        exp: expires_at.timestamp() as usize,
        jti: jti.to_owned(),
    };
    Ok(encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(config.jwt_secret.as_bytes()),
    )?)
}

fn parse_user_id(claims: &JwtClaims) -> Result<Uuid, AuthError> {
    Uuid::parse_str(&claims.sub).map_err(|_| AuthError::InvalidToken)
}

fn parse_system_role(value: &str) -> Result<SystemRole, AuthError> {
    match value {
        "super_admin" => Ok(SystemRole::SuperAdmin),
        "user" => Ok(SystemRole::User),
        _ => Err(AuthError::InvalidSystemRole),
    }
}

fn hash_jti(jti: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(jti.as_bytes());
    // sha2 0.11 的摘要不再实现 LowerHex,手动转十六进制
    hasher
        .finalize()
        .iter()
        .fold(String::with_capacity(64), |mut hex, byte| {
            hex.push_str(&format!("{byte:02x}"));
            hex
        })
}
