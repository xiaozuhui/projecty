use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .name("uq_jwt_refresh_tokens_jti_hash")
                    .table(JwtRefreshTokens::Table)
                    .col(JwtRefreshTokens::JtiHash)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("uq_jwt_refresh_tokens_jti_hash")
                    .table(JwtRefreshTokens::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum JwtRefreshTokens {
    Table,
    JtiHash,
}
