
use crate::sequence::{*, traits::{Length, Operation}};

#[tokio::test]
async fn insertion() -> anyhow::Result<()> {
    let sequence = Sequence::<i32>::allocate(10).await;
    assert_eq!(sequence.length(), 0);
    assert_eq!(sequence.capacity(), 10);

    for i in 0..10 {
        sequence.insert(i, crate::Candidate::Value(i as i32)).await;
        assert!(sequence.length() == i + 1);
    }

    assert_eq!(sequence.length(), 10);
    assert_eq!(sequence.capacity(), 10);

    sequence.insert(sequence.length(), crate::Candidate::Value(10)).await;
    assert_eq!(sequence.length(), 11);
    assert!(sequence.capacity() >= 11);

    Ok(())
}

#[tokio::test]
async fn insertion_concurrent() -> anyhow::Result<()> {
    let sequence = Arc::new(Sequence::<i32>::allocate(5).await);
    assert_eq!(sequence.length(), 0);
    assert_eq!(sequence.capacity(), 5);

    let mut handles = vec![];

    for i in 0..10 {
        let seq = sequence.clone();
        handles.push(tokio::task::spawn(async move {
            seq.insert(i, crate::Candidate::Value(i as i32)).await
        }));
    }

    let results = futures::future::join_all(handles).await;
    for r in results {
        r?;
    }

    assert_eq!(sequence.length(), 10);
    assert!(sequence.capacity() >= 10);

    Ok(())
}

#[tokio::test]
async fn insertion_stress() -> anyhow::Result<()> {
    let sequence = Sequence::<i32>::allocate(1).await;
    assert_eq!(sequence.length(), 0);
    assert_eq!(sequence.capacity(), 1);

    let mut handles = vec![];

    for i in 0..1000 {
        let seq = sequence.clone();
        handles.push(tokio::task::spawn(async move {
            seq.insert(i, crate::Candidate::Value(i as i32)).await
        }));
    }

    let results = futures::future::join_all(handles).await;
    for r in results {
        r?;
    }

    assert_eq!(sequence.length(), 1000);
    assert!(sequence.capacity() >= 1000);

    Ok(())
}

#[tokio::test]
async fn insertion_at_bounds() -> anyhow::Result<()> {
    let sequence = Sequence::<i32>::allocate(5).await;
    assert_eq!(sequence.length(), 0);
    assert_eq!(sequence.capacity(), 5);
    for i in 0..5 {
        sequence.insert(0, crate::Candidate::Value(i as i32)).await;
        assert_eq!(sequence.length(), i + 1);
    }
    assert_eq!(sequence.length(), 5);
    for i in 0..5 {
        let value = sequence.get(i, true).await;
        assert_eq!(value.as_arc().await.read().await.clone(), (4 - i) as i32);
    }
    Ok(())
}

#[tokio::test]
async fn insertion_resize_efficiency() -> anyhow::Result<()> {
    // Test that inserting at end of full sequence resizes efficiently
    // Initial capacity: 5, length: 5
    // After insert at index 5, capacity should be 10 (not just 6)
    let sequence = Sequence::<i32>::allocate(5).await;
    assert_eq!(sequence.capacity(), 5);
    assert_eq!(sequence.length(), 0);

    // Fill the sequence to capacity
    for i in 0..5 {
        sequence.insert(i, crate::Candidate::Value(i as i32)).await;
    }
    assert_eq!(sequence.length(), 5);
    assert_eq!(sequence.capacity(), 5);

    // Insert at end (index 5) - should trigger smart resize to 10
    sequence.insert(5, crate::Candidate::Value(5)).await;
    assert_eq!(sequence.length(), 6);
    assert_eq!(sequence.capacity(), 10, "Capacity should be 10 after smart resize, not just 6");

    // Verify all values are correct
    for i in 0..6 {
        let value = sequence.get(i, true).await;
        assert_eq!(value.as_arc().await.read().await.clone(), i as i32);
    }

    Ok(())
}
