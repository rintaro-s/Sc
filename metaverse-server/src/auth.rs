// metaverse-server/src/auth.rs
//! JWT認証モジュール

use anyhow::{Context, Result};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

fn jwt_secret() -> String {
    std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "cr-bridge-secret-change-me-in-production".to_string())
}

const TOKEN_EXPIRY_SECS: u64 = 86400; // 24時間

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,        // ユーザーID
    pub name: String,       // 表示名
    pub exp: u64,           // 有効期限 (UNIX timestamp)
    pub iat: u64,           // 発行時刻
    pub roles: Vec<String>, // ロール (admin, user, guest)
}

/// JWTトークンを生成
pub fn generate_token(user_id: &str, display_name: &str, roles: Vec<String>) -> Result<String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("System time error")?
        .as_secs();

    let claims = Claims {
        sub: user_id.to_string(),
        name: display_name.to_string(),
        exp: now + TOKEN_EXPIRY_SECS,
        iat: now,
        roles,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret().as_bytes()),
    )
    .context("Failed to encode JWT")?;

    Ok(token)
}

/// JWTトークンを検証
pub fn verify_token(token: &str) -> Result<Claims> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret().as_bytes()),
        &Validation::default(),
    )
    .context("Invalid JWT token")?;

    Ok(token_data.claims)
}

/// WebSocketハンドシェイク時のトークン検証
pub fn extract_token_from_query(query: &str) -> Option<String> {
    // ?token=xxx のフォーマット
    for pair in query.split('&') {
        if let Some(token) = pair.strip_prefix("token=") {
            return Some(token.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_generation_and_verification() {
        let token = generate_token("user123", "TestUser", vec!["user".to_string()]).unwrap();
        let claims = verify_token(&token).unwrap();
        
        assert_eq!(claims.sub, "user123");
        assert_eq!(claims.name, "TestUser");
        assert_eq!(claims.roles, vec!["user"]);
    }

    #[test]
    fn test_invalid_token() {
        let result = verify_token("invalid.token.here");
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_token_from_query() {
        let query = "token=abc123&world=default";
        assert_eq!(extract_token_from_query(query), Some("abc123".to_string()));
        
        let query2 = "world=default";
        assert_eq!(extract_token_from_query(query2), None);
    }
}
