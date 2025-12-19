
use crate::sequence::{*, traits::*};

#[tokio::test]
async fn set() {
    let sequence = Sequence::<i32>::allocate(10).await;

    // currently, the length -> 0,
    // set on an empty sequence -> error
    let result = sequence.set(0, crate::Candidate::Value(42)).await;
    assert!(result.is_err(), "Setting on empty sequence should error!");

    let _ = sequence.append(crate::Candidate::Value(10)).await;
    assert_eq!(sequence.length(), 1);

    let result = sequence.set(0, crate::Candidate::Value(42)).await;
    assert!(result.is_ok(), "Setting on existing index should work!");
    
    if let Some(old_value) = result.ok() {
        assert!(!old_value.empty());
        assert_eq!(*old_value.as_arc().await.read().await, 10);
    } else {
        assert!(false);
    }

    let new_compat = sequence.get(0, true).await;
    assert!(!new_compat.empty());
    assert_eq!(*new_compat.as_arc().await.read().await, 42);

}

#[tokio::test]
async fn set_stress() -> anyhow::Result<()> {
    let sequence = Sequence::<i32>::allocate(10).await;
    for idx in 0..100 {
        sequence.append(crate::Candidate::Value(idx as i32)).await?;
    }

    let handles: Vec<_> = (0..50).map(|_| {
        let seq_clone = sequence.clone();
        tokio::spawn(async move {
            for _ in 0..20 {
                let index = (rand::random::<u32>() % 100) as usize;
                let new_value = rand::random::<i32>();
                let _ = seq_clone.set(index, crate::Candidate::Value(new_value)).await;
            }
        })
    }).collect();

    let results = futures::future::join_all(handles).await;
    for res in results {
        res?;
    }

    Ok(())
}

#[tokio::test]
#[should_panic]
async fn set_error() {
    let sequence = Sequence::<i32>::allocate(5).await;

    // cannot set at an out-of-bounds index
    let _ = sequence.set(10, crate::Candidate::Value(42)).await.unwrap();
}