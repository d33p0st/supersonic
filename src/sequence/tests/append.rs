
use crate::sequence::prelude::*;

#[tokio::test]
async fn append() -> anyhow::Result<()> {
    let sequence = Sequence::<String>::allocate(1).await;
    sequence.append(Candidate::Value("Hello".to_string())).await?;
    assert_eq!(sequence.length(), 1);

    sequence.append(Candidate::Value("World".to_string())).await?;
    assert_eq!(sequence.length(), 2);

    let first = sequence.get(0, true).await;
    let second = sequence.get(1, true).await;

    {
        let arc_first = first.as_arc().await;
        let read_guard_first = arc_first.read().await;
        assert_eq!(*read_guard_first, "Hello");
    }

    {
        let arc_second = second.as_arc().await;
        let read_guard_second = arc_second.read().await;
        assert_eq!(*read_guard_second, "World");
    }

    Ok(())
}

#[tokio::test]
async fn append_stress() -> anyhow::Result<()> {
    let sequence = Sequence::<i32>::allocate(1).await;
    let handles = (0..100).map(|i| {
        let seq_clone = sequence.clone();
        tokio::spawn(async move {
            for j in 0..100 {
                seq_clone.append(Candidate::Value(i * 100 + j)).await.unwrap();
            }
        })
    }).collect::<Vec<_>>();

    let results = futures::future::join_all(handles).await;
    for res in results {
        res?;
    }

    assert_eq!(sequence.length(), 10000);

    Ok(())
}