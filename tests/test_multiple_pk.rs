use sqlx_macros::Table;

#[derive(Table, Debug, Clone)]
#[table(name = "server_permission")]
pub struct ServerPermission {
    #[table(pk)]
    pub guild_id: i64,
    #[table(pk)]
    pub tag: String,
    pub permission_bit: i64,
}

#[tokio::test]
async fn test_pk() {}
