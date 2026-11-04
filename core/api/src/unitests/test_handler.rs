use super::*;

#[tokio::test]
async fn test_easy_consumer() {
    let hd = LocalHandler::new(|value: &String| {
        assert!("hello".to_string() == *value);
    });
    hd.cb(&"hello".to_string());
}

#[tokio::test]
async fn test_box_consumer() {
    let hd = BoxHandler::new(|value: String| {
        Box::pin(async move {
            assert!("hello".to_string() == value);
        })
    });
    hd.cb("hello".to_string()).await;
}

#[tokio::test]
async fn test_local_box_consumer() {
    let hd = LocalBoxHandler::new(|value: String| {
        Box::pin(async move {
            assert_eq!("hello", value);
        })
    });
    hd.cb("hello".to_string()).await;
}
