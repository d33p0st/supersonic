use crate::sequence::prelude::*;

#[tokio::test]
async fn get() -> anyhow::Result<()> {
    let sequence = Sequence::<i32>::allocate(1).await;
    
    // empty value
    let fetched = sequence.get(0, true).await;
    assert!(fetched.empty());

    // insert value
    sequence.insert(0, Candidate::Value(42)).await;
    let fetched = sequence.get(0, true).await;
    assert!(!fetched.empty());
    assert_eq!(*fetched.as_arc().await.read().await, 42);

    Ok(())
}

#[tokio::test]
async fn get_stress() -> anyhow::Result<()> {
    let sequence = Sequence::<i32>::allocate(1000).await;

    for i in 0..1000 {
        sequence.insert(i, Candidate::Value(i as i32)).await;
    }

    let mut handles = vec![];
    for _ in 0..10 {
        let seq_clone = sequence.clone();
        let handle = tokio::spawn(async move {
            let index = (rand::random::<u32>() % 1000) as usize;
            let fetched = seq_clone.get(index, true).await;
            assert!(!fetched.empty(), "index: {}", index);
            assert_eq!(*fetched.as_arc().await.read().await, index as i32);
        });
        handles.push(handle);
    }

    let results = futures::future::join_all(handles).await;
    for result in results {
        result?;
    }

    Ok(())
}


#[tokio::test]
async fn get_stress_2() -> anyhow::Result<()> {
    let sequence = Sequence::<i32>::allocate(1000).await;

    for i in 0..1000 {
        sequence.insert(i, Candidate::Value(i as i32)).await;
    }

    let mut handles = vec![];
    for _ in 0..10 {
        let seq_clone = sequence.clone();
        let handle = tokio::spawn(async move {
            for j in 0..1000 {
                let fetched = seq_clone.get(j, true).await;
                assert!(!fetched.empty(), "index: {}", j);
                assert_eq!(*fetched.as_arc().await.read().await, j as i32);
            }
        });
        handles.push(handle);
    }

    let results = futures::future::join_all(handles).await;
    for result in results {
        result?;
    }

    Ok(())
}