//! 本地账号密码、密码哈希校验、JWT 签发和 refresh token 生命周期。

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use projecty_entity::{jwt_refresh_tokens, users};
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
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
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
    Ok(MeResponse {
        id: user.id,
        account: user.account,
        display_name: user.display_name,
        system_role: parse_system_role(&user.system_role)?,
    })
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
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_hash_can_be_verified() {
        let hash = hash_password("correct horse battery staple").unwrap();
        assert!(verify_password("correct horse battery staple", &hash));
        assert!(!verify_password("wrong password", &hash));
    }

    #[test]
    fn empty_password_is_rejected() {
        assert!(matches!(hash_password(""), Err(AuthError::InvalidPassword)));
    }

    #[test]
    fn jwt_access_claims_round_trip_and_token_type_is_checked() {
        let config = Config {
            environment: "test".to_owned(),
            bind_host: "127.0.0.1".to_owned(),
            bind_port: 8080,
            database_url: "postgres://unused".to_owned(),
            jwt_secret: "a-test-secret-that-is-long-enough".to_owned(),
            jwt_issuer: "projecty-test".to_owned(),
            access_token_ttl_seconds: 900,
            refresh_token_ttl_seconds: 3600,
        };
        let token = encode_claims(
            &config,
            &Uuid::nil(),
            "tester",
            SystemRole::User,
            TokenType::Access,
            "access-jti",
            Utc::now() + Duration::minutes(5),
        )
        .unwrap();
        let claims = decode_access_claims(&config, &token).unwrap();
        assert_eq!(claims.account, "tester");
        assert_eq!(claims.system_role, SystemRole::User);

        assert!(matches!(
            decode_claims(&config, &token, TokenType::Refresh),
            Err(AuthError::InvalidToken)
        ));
    }
}
