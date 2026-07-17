use prayer_sdk::prelude::*;

async fn ordinary_workflow(sdk: &PrayerSdk) -> Result<(), SdkError> {
    let snapshot = sdk.state().await;
    let observed = snapshot.bot("stable-bot-id")?;
    let bot = sdk.bot(observed.id.as_str()).await?;
    let run = bot.start_actions([Action::Wait { ticks: 1 }]).await?;
    let run_id = run.id().clone();
    let _terminal: ActionRunOutcome = run.wait().await?;
    let recovered = bot.action_run(run_id).await?;
    let _ = recovered.cancel("external fixture cleanup").await?;
    let script = bot.start_script("wait 1;").await?;
    let _ = script.prayerlang();
    let _ = script.cancel("external fixture cleanup").await?;
    Ok(())
}

fn main() {
    let _ = ordinary_workflow;
    let _ = PrayerSdkOptions::default().with_state_directory(".prayer-state");
}
