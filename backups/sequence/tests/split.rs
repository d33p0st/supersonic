
use crate::sequence::prelude::*;

#[tokio::test]
async fn split_reactive() -> anyhow::Result<()> {
    // for this test, we will create a sequence of 10 elements
    // then split it at index 5
    // then verify that the two resulting sequences have the correct elements
    // then change elements in the original sequence and verify that the split sequences are also changed
    // and vice versa.

    let sequence = Sequence::<u32>::allocate(10).await;
    for i in 0..10 {
        sequence.append(Candidate::Value(i)).await?;
    }

    let split_sequence = Reactive::split(&sequence, 5).await;
    let first_half = split_sequence.0;
    let second_half = split_sequence.1;

    assert_eq!(first_half.length(), 5);
    assert_eq!(second_half.length(), 5);

    for i in 0..5 {
        let value = first_half.get(i as usize, true).await;
        assert!(!value.empty());
        assert_eq!(*value.as_arc().await.read().await, i);
    }

    for i in 0..5 {
        let value = second_half.get(i as usize, true).await;
        assert!(!value.empty());
        assert_eq!(*value.as_arc().await.read().await, i + 5);
    }

    // modify original sequence
    for i in 0..10 {
        // have to use modify.. as set is not reactive.
        sequence.modify(i as usize, i * 10).await?;
    }

    for i in 0..5 {
        let value = first_half.get(i as usize, true).await;
        assert!(!value.empty());
        assert_eq!(*value.as_arc().await.read().await, i * 10);
    }

    for i in 0..5 {
        let value = second_half.get(i as usize, true).await;
        assert!(!value.empty());
        assert_eq!(*value.as_arc().await.read().await, (i + 5) * 10);
    }

    // modify split sequences
    for i in 0..5 {
        first_half.modify(i as usize, i * 100).await?;
        second_half.modify(i as usize, (i + 5) * 100).await?;
    }

    for i in 0..10 {
        let value = sequence.get(i as usize, true).await;
        assert!(!value.empty());
        assert_eq!(*value.as_arc().await.read().await, i * 100);
    }

    Ok(())
}


#[tokio::test]
async fn split_nonreactive() -> anyhow::Result<()> {
    // for this test, we will create a sequence of 10 elements
    // then split it at index 5
    // then verify that the two resulting sequences have the correct elements
    // then change elements in the original sequence and verify that the split sequences are also changed
    // and vice versa.

    let sequence = Sequence::<u32>::allocate(10).await;
    for i in 1..11 {
        sequence.append(Candidate::Value(i)).await?;
    }

    let split_sequence = NonReactive::split(&sequence, 5).await;
    let first_half = split_sequence.0;
    let second_half = split_sequence.1;

    assert_eq!(first_half.length(), 5);
    assert_eq!(second_half.length(), 5);

    for i in 0..5 {
        let value = first_half.get(i as usize, true).await;
        assert!(!value.empty());
        assert_eq!(*value.as_arc().await.read().await, i + 1);
    }

    for i in 0..5 {
        let value = second_half.get(i as usize, true).await;
        assert!(!value.empty());
        assert_eq!(*value.as_arc().await.read().await, i + 6);
    }

    // modify original sequence
    for i in 0..10 {
        // have to use modify.. as set is not reactive.
        sequence.modify(i as usize, (i + 1) * 10).await?;
    }

    for i in 0..5 {
        let value = first_half.get(i as usize, true).await;
        assert!(!value.empty());
        assert_ne!(*value.as_arc().await.read().await, (i + 1) * 10);
    }

    for i in 0..5 {
        let value = second_half.get(i as usize, true).await;
        assert!(!value.empty());
        assert_ne!(*value.as_arc().await.read().await, (i + 6) * 10);
    }

    // modify split sequences
    for i in 0..5 {
        first_half.modify(i as usize, (i + 1) * 100).await?;
        second_half.modify(i as usize, (i + 6) * 100).await?;
    }

    for i in 0..10 {
        let value = sequence.get(i as usize, true).await;
        assert!(!value.empty());
        assert_ne!(*value.as_arc().await.read().await, (i + 1) * 100);
    }

    Ok(())
}