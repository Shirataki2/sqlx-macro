use sqlx_macros::Table;

#[derive(Table, PartialEq, Eq, Debug)]
#[table(name = "guild")]
pub struct Guild {
    #[table(pk)]
    pub guild_id: i64,
    pub name: String, 
    pub icon_url: Option<String>,
}

#[tokio::test]
async fn test_notfound() {
    let database_url = std::env::var("DATABASE_URL").unwrap();
    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    let guild = Guild::optional_get(&pool, 10).await.unwrap();
    assert!(guild.is_none());
}
