use crate::*;

#[tokio::test]
#[should_panic]
async fn reject_empty_body() {
    CyclingBodySource::new(Arc::new([]), Duration::from_secs(1), None).unwrap();
}
