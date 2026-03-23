use loongclaw_spec::CliResult;

pub async fn run_multi_channel_serve(
    config_path: Option<&str>,
    session: &str,
    telegram_account: Option<&str>,
    feishu_account: Option<&str>,
) -> CliResult<()> {
    let _ = (config_path, session, telegram_account, feishu_account);
    Ok(())
}
