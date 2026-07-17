use prayer_sdk::{Action, PrayerSdk, PrayerSdkOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let clerk_api_key = std::env::var("SPACEMOLT_CLERK_API_KEY")?;
    let bot_selector = std::env::var("PRAYER_BOT")?;
    let sdk =
        PrayerSdk::connect(PrayerSdkOptions::default().with_clerk_api_key(clerk_api_key)).await?;
    let bot = sdk.bot(bot_selector).await?;

    let run = bot.start_actions([Action::Wait { ticks: 1 }]).await?;
    println!("accepted run {:?}:\n{}", run.id(), run.prayerlang());
    println!("outcome: {:?}", run.wait().await?);
    println!("queue:\n{}", bot.queue().await?.rendered_prayerlang());

    sdk.shutdown().await?;
    Ok(())
}
