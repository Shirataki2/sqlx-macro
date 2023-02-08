use sqlx_macros::Table;

#[derive(Table, Debug, Clone)]
pub struct Dictionary {
    #[table(pk)]
    pub guild_id: i64,
    pub dict: String,
}

#[tokio::test]
async fn test_pk() {
}