
use crate::sequence::prelude::*;

#[tokio::test]
async fn drain() -> anyhow::Result<()> {
    let sequence = Sequence::<u32>::allocate(10).await;
    for i in 0..10 {
        sequence.append(Candidate::Value(i)).await?;
    }

    let drained = sequence.drain(Some(0..5)).await;
    assert_eq!(drained.length(), 5);
    for i in 0..5 {
        let value = drained.get(i as usize, true).await;
        assert!(!value.empty());
        assert_eq!(*value.as_arc().await.read().await, i);
    }

    assert_eq!(sequence.length(), 5);
    for i in 0..5 {
        let value = sequence.get(i as usize, true).await;
        assert!(!value.empty());
        assert_eq!(*value.as_arc().await.read().await, i + 5);
    }

    Ok(())
}


#[tokio::test]
async fn drain_full() -> anyhow::Result<()> {
    let sequence = Sequence::<u32>::allocate(10).await;
    for i in 0..10 {
        sequence.append(Candidate::Value(i)).await?;
    }

    let drained = sequence.drain(None).await;
    assert_eq!(drained.length(), 10);
    for i in 0..10 {
        let value = drained.get(i as usize, true).await;
        assert!(!value.empty());
        assert_eq!(*value.as_arc().await.read().await, i);
    }

    assert_eq!(sequence.length(), 0);

    Ok(())
}

#[tokio::test]
async fn drain_empty() -> anyhow::Result<()> {
    let sequence = Sequence::<u32>::allocate(10).await;
    let drained = sequence.drain(None).await;
    assert_eq!(drained.length(), 0);
    assert_eq!(sequence.length(), 0);
    Ok(())
}

#[tokio::test]
async fn drain_stress() -> anyhow::Result<()> {
    let sequence = Sequence::<u32>::allocate(1000).await;
    for i in 0..1000 {
        sequence.append(Candidate::Value(i)).await?;
    }

    for _ in 0..10 {
        let drained = sequence.drain(Some(0..500)).await;
        assert_eq!(drained.length(), 500);
        for i in 0..500 {
            let value = drained.get(i as usize, true).await;
            assert!(!value.empty());
            assert_eq!(*value.as_arc().await.read().await, i);
        }

        assert_eq!(sequence.length(), 500);
        for i in 0..500 {
            let value = sequence.get(i as usize, true).await;
            assert!(!value.empty());
            assert_eq!(*value.as_arc().await.read().await, i + 500);
        }

        for i in 0..500 {
            sequence.insert(i, Candidate::Value(i as u32)).await;
        }
        assert_eq!(sequence.length(), 1000);
    }

    Ok(())
}
