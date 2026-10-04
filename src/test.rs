use grammers_client::Client;

pub async fn test_fn(client: &Client) -> i64 {
    let me = client.get_me().await.unwrap();
    me.id()
}
