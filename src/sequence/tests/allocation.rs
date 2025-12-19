use crate::sequence::{traits::Length, *};

#[tokio::test]
async fn allocation() {
    let seq = Sequence::<i32>::allocate(3).await;
    assert_eq!(seq.length(), 0);
    assert_eq!(seq.capacity(), 3);

    crate::drop!(seq);

    let seq = Sequence::<i32>::allocate_raw(4).await;
    assert_eq!(seq.length(), 0);
    assert_eq!(seq.capacity(), 4);

    crate::drop!(seq);
}

#[tokio::test]
async fn allocation_concurrent() {
    let mut handles = vec![];
    let sequences: Arc<RwLock<Vec<Arc<Sequence<i32>>>>> = Arc::new(RwLock::new(vec![]));

    for idx in 0..10 {
        let s = sequences.clone();
        handles.push(tokio::task::spawn(async move {
            let sequence = Sequence::<i32>::allocate(idx + 1).await;
            s.write().await.push(sequence);
        }))
    }

    let result = futures::future::join_all(handles).await;
    for r in result {
        r.unwrap();
    }

    assert_eq!(sequences.read().await.len(), 10);

    // since allocations are done in order, we can check capacities like this
    for (idx, seq) in sequences.read().await.iter().enumerate() {
        assert_eq!(seq.length(), 0);
        assert_eq!(seq.capacity(), idx + 1);
    }
}