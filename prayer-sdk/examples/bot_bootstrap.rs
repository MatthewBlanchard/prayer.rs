use prayer_sdk::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let clerk_api_key = std::env::var("SPACEMOLT_CLERK_API_KEY")?;
    let sdk =
        PrayerSdk::connect(PrayerSdkOptions::default().with_clerk_api_key(clerk_api_key)).await?;

    for account in &sdk.startup_report().accounts {
        if !account.ready {
            eprintln!(
                "bot {} is not ready: {}",
                account.username,
                account
                    .error
                    .as_deref()
                    .unwrap_or("unknown startup failure")
            );
        }
    }

    // Snapshot acquisition is async; all queries over the captured state are sync.
    let snapshot = sdk.state().await;
    let miner = snapshot.bot("my-miner")?;
    println!("bot: {} ({:?})", miner.id.as_str(), miner.connection);
    println!("ship: {:?}", miner.state.ship);
    println!("cargo: {:?}", miner.state.cargo);
    println!("location: {:?}", miner.state.location);
    println!(
        "current market: {:?}",
        miner
            .state
            .effective_poi_id()
            .and_then(|poi| snapshot.market(poi))
    );
    println!(
        "known systems: {}",
        snapshot.world.state.galaxy.system_records.len()
    );
    println!(
        "known station markets: {}",
        snapshot.world.state.station_markets.len()
    );
    // Live handles coordinate with the runtime and therefore remain async.
    let bot = sdk.bot("my-miner").await?;
    let outcome = bot
        .execute_actions([
            Action::Undock,
            Action::Go {
                destination: GoTarget::Poi("sol_central".into()),
            },
            Action::Dock,
        ])
        .await?;
    println!("action outcome: {outcome:?}");

    let run = bot.start_script("go station-sol;\ndock;").await?;
    println!("queued PrayerLang:\n{}", run.prayerlang());
    println!("script outcome: {:?}", run.wait().await?);
    println!("queue status: {:?}", bot.queue().await?);

    sdk.shutdown().await?;
    Ok(())
}
