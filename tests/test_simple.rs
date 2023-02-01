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
async fn test_simple() {
    let database_url = std::env::var("DATABASE_URL").unwrap();
    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    let guild = Guild {
        guild_id: 1,
        name: "test".to_string(),
        icon_url: None,
    };
    let g1 = guild.create(&pool).await.unwrap();

    let mut g2 = Guild::get(&pool, 1).await.unwrap();

    assert_eq!(g1, g2);

    g2.name = "test2".to_string();

    let g3 = g2.update(&pool).await.unwrap();

    let g4 = Guild::get(&pool, 1).await.unwrap();

    assert_eq!(g3, g4);

    assert_ne!(g1, g4);

    assert_eq!(g4.name, "test2");

    g4.delete(&pool).await.unwrap();
}
