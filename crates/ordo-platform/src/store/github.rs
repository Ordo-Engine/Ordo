use super::*;

impl PlatformStore {
    #[allow(clippy::too_many_arguments)]
    pub async fn save_github_connection(
        &self,
        user_id: &str,
        github_user_id: i64,
        login: &str,
        name: Option<&str>,
        avatar_url: Option<&str>,
        access_token: &str,
        scope: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO github_connections
                 (user_id, github_user_id, login, name, avatar_url, access_token, scope, connected_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())
             ON CONFLICT (user_id) DO UPDATE SET
                 github_user_id = EXCLUDED.github_user_id,
                 login          = EXCLUDED.login,
                 name           = EXCLUDED.name,
                 avatar_url     = EXCLUDED.avatar_url,
                 access_token   = EXCLUDED.access_token,
                 scope          = EXCLUDED.scope,
                 connected_at   = NOW()",
        )
        .bind(user_id)
        .bind(github_user_id)
        .bind(login)
        .bind(name)
        .bind(avatar_url)
        .bind(access_token)
        .bind(scope)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_github_connection(
        &self,
        user_id: &str,
    ) -> Result<Option<crate::github::GitHubConnectionRow>> {
        let row = sqlx::query(
            "SELECT user_id, github_user_id, login, name, avatar_url, connected_at
             FROM github_connections WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| crate::github::GitHubConnectionRow {
            user_id: r.get("user_id"),
            github_user_id: r.get("github_user_id"),
            login: r.get("login"),
            name: r.get("name"),
            avatar_url: r.get("avatar_url"),
            connected_at: r.get("connected_at"),
        }))
    }

    pub async fn get_github_token(&self, user_id: &str) -> Result<Option<String>> {
        let row = sqlx::query("SELECT access_token FROM github_connections WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| r.get("access_token")))
    }

    pub async fn delete_github_connection(&self, user_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM github_connections WHERE user_id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
