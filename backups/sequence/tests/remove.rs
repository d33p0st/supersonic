
use crate::sequence::prelude::*;

#[tokio::test]
async fn remove() -> anyhow::Result<()> {
    let sequence = Sequence::<i32>::allocate(1).await;
    sequence.append(Candidate::Value(42)).await?;
    let fetched = sequence.get(0, true).await;
    assert_eq!(*fetched.as_arc().await.read().await, 42);

    let removed = sequence.remove(0).await?;
    assert_eq!(*removed.as_arc().await.read().await, 42);

    let fetched_after_remove = sequence.get(0, true).await;
    assert!(fetched_after_remove.empty());

    Ok(())
}

#[tokio::test]
async fn remove_out_of_bounds() -> anyhow::Result<()> {
    let sequence = Sequence::<i32>::allocate(1).await;
    sequence.append(Candidate::Value(42)).await?;

    if let Err(_) = sequence.remove(1).await {
        assert!(true);
    } else {
        assert!(false, "Expected error when removing out of bounds");
    }

    Ok(())
}

#[tokio::test]
async fn remove_stress() -> anyhow::Result<()> {
    let sequence = Sequence::<i32>::allocate(1).await;

    for i in 0..1000 {
        sequence.append(Candidate::Value(i)).await?;
    }

    let handles = (0..1000)
        .map(|i | {
            let seq_clone = sequence.clone();
            tokio::spawn(async move {
                let removed = seq_clone.remove(0).await.unwrap(); // pop from front
                assert!(!removed.empty());
                assert_eq!(*removed.as_arc().await.read().await, i as i32); // check the value based on insertion order
            })
        }).collect::<Vec<_>>();
    
    let results = futures::future::join_all(handles).await;
    for res in results {
        res?;
    }


    Ok(())
}