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
}
