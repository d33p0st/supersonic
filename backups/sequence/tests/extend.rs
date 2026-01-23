use crate::sequence::prelude::*;

#[tokio::test]
async fn extend() -> anyhow::Result<()> {
    let sequence = Sequence::<u32>::allocate(5).await;
    for i in 0..5 {
        sequence.append(Candidate::Value(i)).await?;
    }

    sequence.extend(5..10).await;
    assert_eq!(sequence.length(), 10);

    for i in 0..10 {
        let value = sequence.get(i as usize, true).await;
        assert!(!value.empty());
        assert_eq!(*value.as_arc().await.read().await, i);
    }

    Ok(())
}

#[tokio::test]
async fn extend_empty() -> anyhow::Result<()> {
    let sequence = Sequence::<u32>::allocate(5).await;
    sequence.extend(std::iter::empty::<u32>()).await;
    assert_eq!(sequence.length(), 0);
    Ok(())
}

#[tokio::test]
async fn extend_large() -> anyhow::Result<()> {
    let sequence = Sequence::<u32>::allocate(5).await;
    let large_iter = 0..10000;
    sequence.extend(large_iter).await;
    assert_eq!(sequence.length(), 10000);
    for i in 0..10000 {
        let value = sequence.get(i as usize, true).await;
        assert!(!value.empty());
        assert_eq!(*value.as_arc().await.read().await, i);
    }
    Ok(())
}

#[tokio::test]
async fn extend_multiple_times() -> anyhow::Result<()> {
    let sequence = Sequence::<u32>::allocate(2).await;
    for _ in 0..5 {
        sequence.extend(0..3).await;
    }
    assert_eq!(sequence.length(), 15);
    for i in 0..15 {
        let value = sequence.get(i as usize, true).await;
        assert!(!value.empty());
        assert_eq!(*value.as_arc().await.read().await, (i % 3) as u32);
    }
    Ok(())
}

#[tokio::test]
async fn extend_with_gaps() -> anyhow::Result<()> {
    let sequence = Sequence::<u32>::allocate(5).await;
    for i in 0..5 {
        sequence.append(Candidate::Value(i)).await?;
    }
    sequence.extend(vec![10, 11, 12, 13, 14]).await;
    assert_eq!(sequence.length(), 10);
    for i in 0..5 {
        let value = sequence.get(i as usize, true).await;
        assert!(!value.empty());
        assert_eq!(*value.as_arc().await.read().await, i);
    }
    for i in 5..10 {
        let value = sequence.get(i as usize, true).await;
        assert!(!value.empty());
        assert_eq!(*value.as_arc().await.read().await, (i + 5) as u32);
    }
    Ok(())
}

// #[tokio::test]
// async fn extend_stress() -> anyhow::Result<()> {
//     let sequence = Sequence::<u32>::allocate(1).await;
//     let iterations = 100;
//     let items_per_iteration = 1000;

//     for iter in 0..iterations {
//         let start = iter * items_per_iteration;
//         let end = start + items_per_iteration;
//         sequence.extend(start as u32..end as u32).await;
//     }

//     assert_eq!(sequence.length(), iterations * items_per_iteration);

//     for i in 0..(iterations * items_per_iteration) {
//         let value = sequence.get(i as usize, true).await;
//         assert!(!value.empty());
//         assert_eq!(*value.as_arc().await.read().await, i as u32);
//     }

//     Ok(())
// }

#[tokio::test]
async fn extend_stress() -> anyhow::Result<()> {
    let sequence = Sequence::<u32>::allocate(10).await;
    // extend in different threads
    let handles: Vec<_> = (0..10).map(|iter| {
        let seq_clone = sequence.clone();
        tokio::spawn(async move {
            let start = iter * 1000;
            let end = start + 1000;
            seq_clone.extend(start as u32..end as u32).await;
        })
    }).collect();

    let results = futures::future::join_all(handles).await;
    for res in results {
        res?;
    }

    assert_eq!(sequence.length(), 10000);
    // verify all values are present
    let mut present = vec![false; 10000];
    for i in 0..10000 {
        let value = sequence.get(i as usize, true).await;
        assert!(!value.empty());
        let v = *value.as_arc().await.read().await;
        present[v as usize] = true;
    }
    
    for p in present {
        assert!(p);
    }

    Ok(())
}