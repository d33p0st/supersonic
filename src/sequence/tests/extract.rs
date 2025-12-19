
use crate::sequence::prelude::*;

#[tokio::test]
async fn extract_reactive() -> anyhow::Result<()> {
    let sequence = Sequence::<u32>::allocate(10).await;
    for i in 0..10 {
        sequence.append(Candidate::Value(i)).await?;
    }

    let extracted = Reactive::extract(&sequence, Some(2..7)).await;
    assert_eq!(extracted.length(), 5);

    // for index 2-6 in extracted, should be 2-6 in original sequence
    // also, change in the extracted should reflect in the original sequence
    for i in 0..5 {
        let value = extracted.get(i as usize, true).await;
        assert!(!value.empty());
        assert_eq!(*value.as_arc().await.read().await, i + 2);
    }

    // modify extracted
    for i in 0..5 {
        extracted.modify(i as usize, (i + 2) * 10).await?;
    }

    // check original sequence reflects changes
    for i in 2..7 {
        let value = sequence.get(i as usize, true).await;
        assert!(!value.empty());
        assert_eq!(*value.as_arc().await.read().await, i * 10);
    }

    // modify original sequence
    for i in 2..7 {
        sequence.modify(i as usize, i + 20).await?;
    }
    // check extracted reflects changes
    for i in 0..5 {
        let value = extracted.get(i as usize, true).await;
        assert!(!value.empty());
        assert_eq!(*value.as_arc().await.read().await, (i + 2) + 20);
    }

    Ok(())
}

#[tokio::test]
async fn extract_nonreactive() -> anyhow::Result<()> {
    let sequence = Sequence::<u32>::allocate(10).await;
    for i in 0..10 {
        sequence.append(Candidate::Value(i)).await?;
    }

    let extracted = NonReactive::extract(&sequence, Some(2..7)).await;
    assert_eq!(extracted.length(), 5);

    // for index 2-6 in extracted, should be 2-6 in original sequence
    // also, change in the extracted should reflect in the original sequence
    for i in 0..5 {
        let value = extracted.get(i as usize, true).await;
        assert!(!value.empty());
        assert_eq!(*value.as_arc().await.read().await, i + 2);
    }

    // modify extracted
    for i in 0..5 {
        extracted.modify(i as usize, (i + 2) * 10).await?;
    }

    // check original sequence reflects changes
    for i in 2..7 {
        let value = sequence.get(i as usize, true).await;
        assert!(!value.empty());
        assert_ne!(*value.as_arc().await.read().await, i * 10);
    }

    // modify original sequence
    for i in 2..7 {
        sequence.modify(i as usize, i + 20).await?;
    }
    // check extracted reflects changes
    for i in 0..5 {
        let value = extracted.get(i as usize, true).await;
        assert!(!value.empty());
        assert_ne!(*value.as_arc().await.read().await, (i + 2) + 20);
    }

    Ok(())
}