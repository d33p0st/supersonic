
use crate::sequence::prelude::*;

#[tokio::test]
async fn modify() -> anyhow::Result<()> {
    let sequence = Sequence::<u32>::allocate(5).await;
    for i in 0..5 {
        sequence.append(Candidate::Value(i)).await?;
    }

    // for this test,
    // we will use the modify function to increment each value by 10
    for i in 0..5 {
        sequence.modify(i, i as u32 + 10).await?;
    }

    for i in 0..5 {
        let value = sequence.get(i as usize, true).await;
        assert!(!value.empty());
        assert_eq!(*value.as_arc().await.read().await, i + 10);
    }
    
    Ok(())
}